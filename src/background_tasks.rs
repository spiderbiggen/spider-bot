use serenity::client::Context;
use serenity::model::prelude::ChannelId;
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{error, info};

use otaku::{DownloadCollection, Subscriber};

use crate::consts::CACHE_TRIM_INTERVAL;
use crate::SpiderBot;

/// Core task spawning function. Creates a set of periodically recurring tasks on their own threads.
///
/// ### Arguments
///
/// - `bot` - the bot instance to delegate to tasks
pub(crate) fn start_background_tasks(bot: &SpiderBot, context: Context) {
    let (tx, rx) = channel(16);

    tokio::spawn(trim_cache(bot.gif_cache.clone()));
    tokio::spawn(otaku::subscribe(bot.config.anime_url, bot.pool.clone(), tx));
    tokio::spawn(embed_sender(context, rx));
}

async fn trim_cache<T>(cache: crate::cache::Memory<T>) -> anyhow::Result<()>
where
    T: ?Sized,
{
    let mut interval = tokio::time::interval(CACHE_TRIM_INTERVAL);
    loop {
        interval.tick().await;
        cache.trim().await;
    }
}

async fn embed_sender(context: Context, mut rx: Receiver<DownloadCollection>) {
    loop {
        if let Some(message) = rx.recv().await {
            let channel_ids = message
                .subscribers
                .iter()
                .filter_map(|s| match s {
                    Subscriber::User(_) => None,
                    Subscriber::Channel { channel_id, .. } => Some(channel_id),
                })
                .map(|&id| ChannelId(id));

            let title = message.episode.to_string();
            let fields = message.downloads.iter().map(|d| {
                (
                    d.resolution.clone(),
                    format!("[torrent]({})\n[comments]({})", d.torrent, d.comments),
                    true,
                )
            });

            let mut success_count = 0usize;
            for channel_id in channel_ids {
                let result = channel_id
                    .send_message(&context.http, |m| {
                        m.embed(|e| e.title(&title).fields(fields.clone()))
                    })
                    .await;

                match result {
                    Ok(_) => success_count += 1,
                    Err(err) => {
                        let channel = context.cache.channel(channel_id);
                        error!(
                            r#"Failed to send message for "{}" to {:?}. Due to {}"#,
                            title, channel, err
                        );
                    }
                }
            }

            if success_count > 0 {
                info!(
                    r#"Successfully notified {:?} channels for: "{}"#,
                    success_count, title
                );
            }
        }
    }
}
