use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serenity::all::{CacheHttp, CreateMessage, Message, UserId};
use serenity::builder::{Builder, CreateEmbed};
use serenity::cache::Cache;
use serenity::http::Http;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info, instrument};
use url::Url;

use otaku::db::Pool;
use otaku::{Download, DownloadCollection, Subscribed, Subscriber};

use crate::cache;
use crate::commands::gifs;
use crate::consts::CACHE_TRIM_INTERVAL;

fn interval_at_previous_period(period: Duration) -> anyhow::Result<Interval> {
    let start = Instant::now();
    let now: DateTime<Utc> = Utc::now();
    let seconds = u64::try_from(now.timestamp())?;
    let sub_seconds = seconds % period.as_secs();
    let minute = DateTime::from_timestamp(i64::try_from(seconds - sub_seconds)?, 0)
        .ok_or(anyhow!("failed to create new date time"))?;
    let offset = (now - minute).to_std()?;
    let best_effort_start = start.checked_sub(offset).unwrap_or(start);
    Ok(interval_at(best_effort_start, period))
}

pub(crate) fn start_sleep_gif_updater(
    tenor: tenor::Client,
    gif_cache: cache::Memory<[Url]>,
) -> anyhow::Result<()> {
    let mut interval = interval_at_previous_period(Duration::from_secs(6 * 3600))?;
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            if let Err(err) = gifs::update_sleep_cache(&tenor, &gif_cache).await {
                error!("failed to update sleep gif cache: {}", err);
            }
        }
    });
    Ok(())
}

/// Launch a periodic trim of the gif cache.
///
/// ### Arguments
///
/// - `gif_cache` - the cache of gifs
pub(crate) fn start_cache_trim(gif_cache: cache::Memory<[Url]>) {
    let mut interval = tokio::time::interval(CACHE_TRIM_INTERVAL);
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            gif_cache.trim().await;
        }
    });
}

/// Subscribe to announcements of new anime episodes from the anime api.
///
/// ### Arguments
///
/// - `pool` - the database connection pool
/// - `anime_url` - the base url of the anime api
/// - `discord` - the discord http client and cache
pub(crate) fn start_anime_subscription(
    pool: Pool,
    anime_url: &'static str,
    discord_cache: Arc<Cache>,
    discord_http: Arc<Http>,
) {
    let (tx, rx) = channel(16);

    tokio::spawn(otaku::subscribe(anime_url, pool, tx));
    tokio::spawn(embed_sender(discord_cache, discord_http, rx));
}

async fn embed_sender(
    discord_cache: Arc<Cache>,
    discord_http: Arc<Http>,
    mut rx: Receiver<Subscribed<DownloadCollection>>,
) {
    loop {
        if let Some(message) = rx.recv().await {
            tokio::spawn(process_downloads_subscription(
                discord_cache.clone(),
                discord_http.clone(),
                message,
            ));
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum MessageChannelId {
    User(UserId),
    Guild(GuildId, ChannelId),
}

impl MessageChannelId {
    async fn send_message(
        self,
        cache_http: impl CacheHttp,
        builder: CreateMessage,
    ) -> Result<Message, serenity::Error> {
        match self {
            MessageChannelId::User(id) => id.direct_message(cache_http, builder).await,
            MessageChannelId::Guild(guild_id, channel_id) => {
                builder
                    .execute(cache_http, (channel_id, Some(guild_id)))
                    .await
            }
        }
    }
}

#[instrument(skip_all, fields(title))]
async fn process_downloads_subscription(
    discord_cache: Arc<Cache>,
    discord_http: Arc<Http>,
    message: Subscribed<DownloadCollection>,
) {
    let channel_ids = channel_ids(&message.subscribers);

    let title = format!("{} {}", message.content.title, message.content.variant);
    tracing::Span::current().record("title", &title);
    let embed = create_embed(title, message.content.downloads, message.content.created_at);

    info!("Notifying {} channels", channel_ids.len());
    for channel_id in channel_ids {
        if let Err(err) = send_embed(&discord_http, channel_id, embed.clone()).await {
            error!(
                "Failed to send embed to `{}`: {err}",
                format_channel(&discord_cache, channel_id),
            );
        }
    }
}

fn create_embed(title: String, downloads: Vec<Download>, timestamp: DateTime<Utc>) -> CreateEmbed {
    let mut embed = CreateEmbed::new().title(title).timestamp(timestamp);
    for d in downloads {
        embed = embed.field(
            format!("{}p", d.resolution),
            format!("[torrent]({})\n[comments]({})", d.torrent, d.comments),
            true,
        );
    }

    embed
}

fn channel_ids(subscribers: &[Subscriber]) -> impl ExactSizeIterator<Item = MessageChannelId> + '_ {
    subscribers.iter().map(|&s| match s {
        Subscriber::User(id) => MessageChannelId::User(id.into()),
        Subscriber::Channel {
            guild_id,
            channel_id,
        } => MessageChannelId::Guild(guild_id.into(), channel_id.into()),
    })
}

async fn send_embed(
    http: &Http,
    channel_id: ChannelId,
    embed: CreateEmbed,
) -> Result<(), serenity::Error> {
    channel_id
        .send_message(http, CreateMessage::new().embed(embed))
        .await?;
    Ok(())
}

fn format_channel(cache: &Cache, id: MessageChannelId) -> String {
    match id {
        MessageChannelId::User(id) => cache
            .user(id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| id.to_string()),
        MessageChannelId::Guild(guild_id, channel_id) => {
            let Some(guild) = cache.guild(guild_id) else {
                return format!("{guild_id} #{channel_id}");
            };
            let Some(channel) = guild.channels.get(&channel_id) else {
                return format!("{} #{channel_id}", guild.name);
            };
            format!("{} #{}", guild.name, channel.name)
        }
    }
}
