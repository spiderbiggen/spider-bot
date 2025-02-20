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
use tokio::sync::mpsc::{Receiver, channel};
use tokio::time::{Instant, Interval, interval_at};
use tracing::{error, info, instrument};
use url::Url;

use otaku::db::Pool;
use otaku::{Download, DownloadCollection, Subscribed, Subscriber};

use crate::cache;
use crate::commands::gifs;
use crate::consts::SHORT_CACHE_LIFETIME;

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

pub(crate) fn start_gif_updater(
    tenor: tenor::Client<'static>,
    gif_cache: cache::Memory<[Url]>,
) -> anyhow::Result<()> {
    let context = (tenor, gif_cache);
    let mut interval = interval_at_previous_period(Duration::from_secs(6 * 3600))?;
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            gifs::update_gif_cache(&context).await;
        }
    });
    Ok(())
}

/// Launch periodic trim of the GIF cache.
///
/// ### Arguments
///
/// - `gif_cache` - the cache of GIFs
pub(crate) fn start_cache_trim(gif_cache: cache::Memory<[Url]>) {
    let mut interval = tokio::time::interval(SHORT_CACHE_LIFETIME);
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

    async fn send_embed(
        self,
        cache_http: impl CacheHttp,
        embed: CreateEmbed,
    ) -> Result<Message, serenity::Error> {
        self.send_message(cache_http, CreateMessage::new().embed(embed))
            .await
    }

    fn format(self, cache: &Cache) -> String {
        match self {
            MessageChannelId::User(id) => cache
                .user(id)
                .map_or_else(|| id.to_string(), |s| s.name.clone()),
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
}

#[instrument(skip_all, fields(title))]
async fn process_downloads_subscription(
    discord_cache: Arc<Cache>,
    discord_http: Arc<Http>,
    message: Subscribed<DownloadCollection>,
) {
    let title = format!("{} {}", message.content.title, message.content.variant);
    tracing::Span::current().record("title", &title);

    let embed = CreateEmbed::new()
        .title(title)
        .timestamp(message.content.created_at)
        .fields(download_fields(message.content.downloads));

    let channel_ids = channel_ids(&message.subscribers);
    info!("Notifying {} channels", channel_ids.len());
    for channel_id in channel_ids {
        if let Err(err) = channel_id.send_embed(&discord_http, embed.clone()).await {
            error!(
                channel_id = channel_id.format(&discord_cache),
                "Failed to send embed to, {err}",
            );
        }
    }
}

fn download_fields<I>(downloads: I) -> impl IntoIterator<Item = (String, String, bool)>
where
    I: IntoIterator<Item = Download>,
{
    downloads.into_iter().map(|download| {
        (
            format!("{}p", download.resolution),
            format!(
                "[torrent]({})\n[comments]({})",
                download.torrent, download.comments
            ),
            true,
        )
    })
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
