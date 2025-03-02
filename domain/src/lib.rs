use chrono::{DateTime, Utc};
use std::fmt::Display;
use std::num::NonZeroU64;
use std::ops::RangeInclusive;

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    pub number: u32,
    pub decimal: Option<u32>,
    pub version: Option<u32>,
    pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub published_date: DateTime<Utc>,
    pub resolution: u16,
    pub comments: String,
    pub torrent: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadCollection {
    pub title: String,
    pub variant: DownloadVariant,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub downloads: Vec<Download>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadVariant {
    Batch(RangeInclusive<u32>),
    Episode(Episode),
    Movie,
}

impl Display for DownloadVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadVariant::Batch(range) => {
                write!(f, "[Batch ({}-{})", range.start(), range.end())
            }
            DownloadVariant::Episode(episode) => {
                write!(f, "[Ep {}", episode.number)?;
                if let Some(decimal) = episode.decimal {
                    write!(f, ".{decimal}",)?;
                }
                if let Some(version) = episode.version {
                    write!(f, "v{version}")?;
                }
                if let Some(extra) = &episode.extra {
                    write!(f, "{extra}")?;
                }
                write!(f, "]")
            }
            DownloadVariant::Movie => write!(f, "[Movie]"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subscribed<T: Clone + PartialEq> {
    pub content: T,
    pub subscribers: Vec<Subscriber>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Subscriber {
    User(NonZeroU64),
    Channel {
        channel_id: NonZeroU64,
        guild_id: NonZeroU64,
    },
}

pub struct UserBalance {
    pub user_id: u64,
    pub balance: i64,
}
