use std::sync::Arc;

use serenity::builder::CreateEmbed;
use serenity::cache::Cache;
use serenity::http::Http;
use serenity::model::channel::Channel;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::CacheAndHttp;
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{error, info, instrument};

use otaku::db::Pool;
use otaku::{Download, DownloadCollection, Subcribed, Subscriber};
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
    discord: Arc<CacheAndHttp>,
) {
    let (tx, rx) = channel(16);

    tokio::spawn(otaku::subscribe(anime_url, pool, tx));
    tokio::spawn(embed_sender(discord, rx));
}

async fn embed_sender(discord: Arc<CacheAndHttp>, mut rx: Receiver<Subcribed<DownloadCollection>>) {
    loop {
        if let Some(message) = rx.recv().await {
            tokio::spawn(process_downloads_subscription(discord.clone(), message));
        }
    }
}

#[instrument(skip_all, fields(title))]
async fn process_downloads_subscription(
    discord: Arc<CacheAndHttp>,
    message: Subcribed<DownloadCollection>,
) {
    let channel_ids = channel_ids(&message.subscribers);

    let title = message.content.episode.to_string();
    tracing::Span::current().record("title", &title);
    let embed = create_embed(&title, message.content.downloads);

    info!("Notifying {} channels", channel_ids.len());
    for channel_id in channel_ids {
        if let Err(err) = send_embed(&discord.http, channel_id, embed.clone()).await {
            error!(
                "Failed to send embed to `{}`: {err}",
                format_channel(&discord.cache, channel_id),
            );
        }
    }
}

fn create_embed(title: &str, downloads: Vec<Download>) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title(title);
    for d in downloads {
        embed.field(
            d.resolution,
            format!("[torrent]({})\n[comments]({})", d.torrent, d.comments),
            true,
        );
    }

    embed
}

fn channel_ids(subscribers: &[Subscriber]) -> impl ExactSizeIterator<Item = ChannelId> + '_ {
    subscribers.iter().map(|s| match s {
        Subscriber::User(id) => ChannelId(*id),
        Subscriber::Channel { channel_id, .. } => ChannelId(*channel_id),
    })
}

async fn send_embed(
    http: &Http,
    channel_id: ChannelId,
    embed: CreateEmbed,
) -> Result<(), serenity::Error> {
    channel_id
        .send_message(http, |m| m.set_embed(embed))
        .await?;
    Ok(())
}

fn format_channel(cache: &Cache, channel_id: ChannelId) -> String {
    match channel_id.to_channel_cached(cache) {
        Some(Channel::Guild(channel)) => {
            let guild_id = format_guild(cache, channel.guild_id);
            format!("{} #{}", guild_id, channel.name)
        }
        Some(Channel::Category(category)) => {
            let guild_id = format_guild(cache, category.guild_id);
            format!("{} #{}", guild_id, category.name)
        }
        Some(Channel::Private(channel)) => channel.name(),
        _ => channel_id.to_string(),
    }
}

fn format_guild(cache: &Cache, guild_id: GuildId) -> String {
    match guild_id.name(cache) {
        Some(name) => name,
        None => guild_id.to_string(),
    }
}
