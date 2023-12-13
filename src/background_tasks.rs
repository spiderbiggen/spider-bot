use std::sync::Arc;

use serenity::all::CreateMessage;
use serenity::builder::CreateEmbed;
use serenity::cache::Cache;
use serenity::http::Http;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{error, info, instrument};

use otaku::db::Pool;
use otaku::{Download, DownloadCollection, Subscribed, Subscriber};
use tenor::models::Gif;

use crate::cache;
use crate::consts::CACHE_TRIM_INTERVAL;

/// Launch a periodic trim of the gif cache.
///
/// ### Arguments
///
/// - `gif_cache` - the cache of gifs
pub(crate) fn start_cache_trim(gif_cache: cache::Memory<[Gif]>) {
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

#[instrument(skip_all, fields(title))]
async fn process_downloads_subscription(
    discord_cache: Arc<Cache>,
    discord_http: Arc<Http>,
    message: Subscribed<DownloadCollection>,
) {
    let channel_ids = channel_ids(&message.subscribers);

    let title = message.content.episode.to_string();
    tracing::Span::current().record("title", &title);
    let embed = create_embed(&title, message.content.downloads);

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

fn create_embed(title: &str, downloads: Vec<Download>) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed = embed.title(title);
    for d in downloads {
        embed = embed.field(
            d.resolution,
            format!("[torrent]({})\n[comments]({})", d.torrent, d.comments),
            true,
        );
    }

    embed
}

fn channel_ids(subscribers: &[Subscriber]) -> impl ExactSizeIterator<Item = ChannelId> + '_ {
    subscribers.iter().map(|&s| match s {
        Subscriber::User(id) => id.into(),
        Subscriber::Channel { channel_id, .. } => channel_id.into(),
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

fn format_channel(cache: &Cache, channel_id: ChannelId) -> String {
    let Some(channel_ref) = channel_id.to_channel_cached(cache) else {
        return channel_id.to_string();
    };
    let guild_id = format_guild(cache, channel_ref.guild_id);
    format!("{guild_id} #{}", channel_ref.name)
}

fn format_guild(cache: &Cache, guild_id: GuildId) -> String {
    guild_id.name(cache).unwrap_or_else(|| guild_id.to_string())
}
