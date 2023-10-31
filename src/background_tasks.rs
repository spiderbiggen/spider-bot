use std::sync::Arc;

use serenity::builder::CreateEmbed;
use serenity::http::Http;
use serenity::model::prelude::ChannelId;
use serenity::CacheAndHttp;
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{info, instrument};

use otaku::db::Pool;
use otaku::{Download, DownloadCollection, Subscriber};
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

async fn embed_sender(discord: Arc<CacheAndHttp>, mut rx: Receiver<DownloadCollection>) {
    loop {
        if let Some(message) = rx.recv().await {
            let channel_ids = channel_ids(&message.subscribers);
            let title = message.episode.to_string();
            info!(r#"Notifiying channels for: "{title}"#,);
            let embed = create_embed(&title, message.downloads);

            for channel_id in channel_ids {
                tokio::spawn(send_embed(discord.http.clone(), channel_id, embed.clone()));
            }
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

fn channel_ids(subscribers: &[Subscriber]) -> impl Iterator<Item = ChannelId> + '_ {
    subscribers
        .iter()
        .filter_map(|s| match s {
            Subscriber::User(_) => None,
            Subscriber::Channel { channel_id, .. } => Some(channel_id),
        })
        .map(|&id| ChannelId(id))
}

#[instrument(skip(http), err(Debug))]
async fn send_embed(
    http: Arc<Http>,
    channel_id: ChannelId,
    embed: CreateEmbed,
) -> Result<(), serenity::Error> {
    channel_id
        .send_message(http, |m| m.set_embed(embed.clone()))
        .await?;
    Ok(())
}
