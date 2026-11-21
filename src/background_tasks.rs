use std::sync::Arc;
use std::time::Duration;

use jiff::{Timestamp as JiffTimestamp, ToSpan};
use klipy::Klipy;
use serenity::Client as Serenity;
use serenity::all::{CacheHttp, CreateEmbed, CreateMessage, Message, Timestamp, UserId};
use serenity::cache::Cache;
use serenity::http::Http;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use tokio::sync::mpsc::{Receiver, channel};
use tokio::time::{Instant, Interval, interval_at};
use tracing::instrument;

use db::BotDatabase;
use domain::{Download, DownloadCollection, Subscribed, Subscriber};

use crate::cache::GifCacheWriter;
use crate::consts::SHORT_CACHE_LIFETIME;
use crate::interactions::gifs;

fn interval_at_previous_period(period: Duration) -> anyhow::Result<Interval> {
    anyhow::ensure!(
        !period.is_zero() && period.subsec_nanos() == 0,
        "interval period must be a positive whole number of seconds"
    );
    let start = Instant::now();
    let seconds = u64::try_from(JiffTimestamp::now().as_second())?;
    let offset = Duration::from_secs(seconds % period.as_secs());
    let best_effort_start = start.checked_sub(offset).unwrap_or(start);
    Ok(interval_at(best_effort_start, period))
}

pub(crate) fn start_gif_updater(
    klipy: Klipy<'static>,
    gif_cache_writer: GifCacheWriter,
) -> anyhow::Result<()> {
    let mut interval = interval_at_previous_period(Duration::try_from(6.hours())?)?;
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            gifs::refresh_global_gif_cache(&klipy, &gif_cache_writer).await;
        }
    });
    Ok(())
}

/// Launch periodic trim of the GIF cache.
///
/// ### Arguments
///
/// - `gif_cache_writer` - the cache writer handle
pub(crate) fn start_cache_trim(gif_cache_writer: GifCacheWriter) {
    let mut interval = tokio::time::interval(SHORT_CACHE_LIFETIME);
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            gif_cache_writer.trim();
        }
    });
}

#[derive(Clone)]
pub(crate) struct DiscordApi(Arc<Http>, Arc<Cache>);

impl From<&Serenity> for DiscordApi {
    fn from(value: &Serenity) -> Self {
        Self(Arc::clone(&value.http), Arc::clone(&value.cache))
    }
}

impl DiscordApi {
    /// Subscribe to announcements of new anime episodes from the anime api.
    ///
    /// ### Arguments
    ///
    /// - `db` - the database connection
    /// - `url` - the base url of the anime api
    pub(crate) fn publish_anime_updates(self, db: BotDatabase, url: &'static str) {
        let (tx, rx) = channel(16);

        tokio::spawn(otaku::subscribe(url, db, tx));
        tokio::spawn(self.embed_sender(rx));
    }

    async fn embed_sender(self, mut rx: Receiver<Subscribed<DownloadCollection>>) {
        while let Some(message) = rx.recv().await {
            self.process_downloads_subscription(message).await;
        }
    }

    #[instrument(skip_all, fields(title))]
    async fn process_downloads_subscription(&self, message: Subscribed<DownloadCollection>) {
        let embed = match build_embed(message.content) {
            Ok(embed) => embed,
            Err(err) => {
                tracing::error!("Failed to create timestamp, {err}");
                return;
            }
        };

        let channel_ids = channel_ids(&message.subscribers);
        tracing::info!("Notifying {} channels", channel_ids.len());
        for channel_id in channel_ids {
            if let Err(err) = channel_id.send_embed(&self.0, embed.clone()).await {
                tracing::error!(
                    channel_id = channel_id.format(&self.1),
                    "Failed to send embed, {err}",
                );
            }
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
        builder: CreateMessage<'_>,
    ) -> Result<Message, serenity::Error> {
        match self {
            MessageChannelId::User(id) => id.direct_message(&cache_http, builder).await,
            MessageChannelId::Guild(_guild_id, channel_id) => {
                builder.execute(cache_http.http(), channel_id.into()).await
            }
        }
    }

    async fn send_embed(
        self,
        cache_http: impl CacheHttp,
        embed: CreateEmbed<'_>,
    ) -> Result<Message, serenity::Error> {
        self.send_message(cache_http, CreateMessage::new().embed(embed))
            .await
    }

    fn format(self, cache: &Cache) -> String {
        match self {
            MessageChannelId::User(id) => id.to_string(),
            MessageChannelId::Guild(guild_id, channel_id) => {
                let Some(guild) = cache.guild(guild_id) else {
                    return format!("{guild_id} #{channel_id}");
                };
                let Some(channel) = guild.channels.get(&channel_id) else {
                    return format!("{} #{channel_id}", guild.name);
                };
                format!("{} #{}", guild.name, channel.base.name)
            }
        }
    }
}

/// Longest overview shown in an embed: roughly two lines on desktop.
const MAX_OVERVIEW_CHARS: usize = 120;

fn build_embed(
    collection: DownloadCollection,
) -> Result<CreateEmbed<'static>, serenity::model::timestamp::InvalidTimestamp> {
    let timestamp = Timestamp::from_millis(collection.created_at.as_millisecond())?;
    let show = collection.show;
    // The show name is cleaner than the release title, but only exists when the title was matched.
    let name = show
        .as_ref()
        .map(|show| show.name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or(&collection.title);
    let title = format!("{name} {}", collection.variant);
    tracing::Span::current().record("title", &title);

    let mut embed = CreateEmbed::new()
        .title(title)
        .timestamp(timestamp)
        .fields(download_fields(collection.downloads));
    if let Some(show) = show {
        if let Some(overview) = show.overview {
            embed = embed.description(truncate_overview(&overview));
        }
        if let Some(poster_url) = show.poster_url {
            embed = embed.thumbnail(poster_url, None);
        }
        if let Some(backdrop_url) = show.backdrop_url {
            embed = embed.image(backdrop_url, None);
        }
    }
    Ok(embed)
}

/// Collapses whitespace and cuts the text to [`MAX_OVERVIEW_CHARS`] on a word boundary.
fn truncate_overview(overview: &str) -> String {
    let text = overview.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= MAX_OVERVIEW_CHARS {
        return text;
    }
    // Room for the ellipsis, so the result never exceeds the limit.
    let end = text
        .char_indices()
        .nth(MAX_OVERVIEW_CHARS - 1)
        .map_or(text.len(), |(index, _)| index);
    let cut = &text[..end];
    // Prefer ending at a word boundary, unless that would throw away most of the text.
    let cut = match cut.rfind(' ') {
        Some(space) if space >= end / 2 => &cut[..space],
        _ => cut,
    };
    format!(
        "{}…",
        cut.trim_end_matches(|c: char| c.is_whitespace() || ",.;:-".contains(c))
    )
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
        Subscriber::User(id) => MessageChannelId::User(UserId::new(id.get())),
        Subscriber::Channel {
            guild_id,
            channel_id,
        } => MessageChannelId::Guild(
            GuildId::new(guild_id.get()),
            ChannelId::new(channel_id.get()),
        ),
    })
}

#[cfg(test)]
mod embed_tests {
    use super::*;
    use domain::{DownloadVariant, Episode, Show};
    use serde_json::Value;

    fn collection(show: Option<Show>) -> DownloadCollection {
        DownloadCollection {
            title: "Release Title S2".into(),
            variant: DownloadVariant::Episode(Episode {
                number: 7,
                decimal: None,
                version: None,
                extra: None,
            }),
            created_at: "2026-11-20T16:02:24Z".parse().unwrap(),
            updated_at: "2026-11-20T16:02:28Z".parse().unwrap(),
            downloads: vec![Download {
                published_date: "2026-11-20T16:02:28Z".parse().unwrap(),
                resolution: 1080,
                comments: "https://nyaa.si/view/1".into(),
                torrent: "https://nyaa.si/download/1.torrent".into(),
                file_name: "f".into(),
            }],
            show,
        }
    }

    fn show() -> Show {
        Show {
            tmdb_id: 1,
            tvdb_id: None,
            name: "Show Name".into(),
            romaji_name: None,
            original_name: None,
            overview: Some("A short overview.".into()),
            poster_url: Some("https://p/x.jpg".into()),
            backdrop_url: Some("https://b/x.jpg".into()),
        }
    }

    fn embed(show: Option<Show>) -> Value {
        serde_json::to_value(build_embed(collection(show)).unwrap()).unwrap()
    }

    #[test]
    fn without_show_uses_release_title_and_no_extras() {
        let embed = embed(None);
        assert_eq!(embed["title"], "Release Title S2 [Ep 7]");
        assert_eq!(embed["fields"][0]["name"], "1080p");
        for key in ["description", "image", "thumbnail"] {
            assert!(embed.get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn with_show_adds_name_description_thumbnail_and_image() {
        let embed = embed(Some(show()));
        assert_eq!(embed["title"], "Show Name [Ep 7]");
        assert_eq!(embed["description"], "A short overview.");
        assert_eq!(embed["thumbnail"]["url"], "https://p/x.jpg");
        assert_eq!(embed["image"]["url"], "https://b/x.jpg");
    }

    #[test]
    fn partial_show_only_adds_what_is_present() {
        let embed = embed(Some(Show {
            overview: None,
            poster_url: None,
            ..show()
        }));
        assert!(embed.get("description").is_none());
        assert!(embed.get("thumbnail").is_none());
        assert_eq!(embed["image"]["url"], "https://b/x.jpg");
    }

    #[test]
    fn blank_show_name_falls_back_to_release_title() {
        let embed = embed(Some(Show {
            name: "  ".into(),
            ..show()
        }));
        assert_eq!(embed["title"], "Release Title S2 [Ep 7]");
    }

    #[test]
    fn short_overview_is_kept_and_whitespace_collapsed() {
        assert_eq!(
            truncate_overview(
                "one

two   three"
            ),
            "one two three"
        );
        let exact = "a".repeat(MAX_OVERVIEW_CHARS);
        assert_eq!(truncate_overview(&exact), exact);
    }

    #[test]
    fn long_overview_is_cut_at_a_word_boundary() {
        let text = "word ".repeat(40);
        let result = truncate_overview(&text);
        assert!(result.ends_with("word…"), "{result}");
        assert!(result.chars().count() <= MAX_OVERVIEW_CHARS);
    }

    #[test]
    fn long_overview_without_spaces_and_multibyte_is_cut_safely() {
        let result = truncate_overview(&"é".repeat(300));
        assert_eq!(result.chars().count(), MAX_OVERVIEW_CHARS);
        assert!(result.ends_with('…'));
    }
}
