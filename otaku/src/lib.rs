#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::cmp::min;
use std::fmt::Display;
use std::num::ParseIntError;
use std::time::Duration;

use futures_util::TryStreamExt;
use prost_types::Timestamp;
use sqlx::pool::Pool;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::Postgres;
use tokio::sync::mpsc::Sender;
use tonic::codec::CompressionEncoding;
use tracing::{debug, error, info};

use proto::api::v1::downloads_client::DownloadsClient;

pub mod db;

const MAX_BACKOFF: Duration = Duration::from_secs(30);
const BACKOFF_INTERVAL: Duration = Duration::from_millis(125);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Encounter invalid timestamp")]
    InvalidTimeStamp(Timestamp),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub title: String,
    pub number: u32,
    pub decimal: Option<u32>,
    pub version: Option<u32>,
    pub extra: Option<String>,
}

impl Display for Episode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} Ep {}", self.title, self.number)?;
        if let Some(decimal) = self.version {
            write!(f, ".{decimal}",)?;
        }
        if let Some(version) = self.version {
            write!(f, "v{version}")?;
        }
        if let Some(extra) = &self.extra {
            write!(f, "{extra}",)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub published_date: DateTime<Utc>,
    pub resolution: String,
    pub comments: String,
    pub torrent: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadCollection {
    pub episode: Episode,
    pub downloads: Vec<Download>,
    pub subscribers: Vec<Subscriber>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subscriber {
    User(u64),
    Channel { channel_id: u64, guild_id: u64 },
}

pub async fn subscribe(endpoint: &str, pool: Pool<Postgres>, sender: Sender<DownloadCollection>) {
    loop {
        let client = connect_with_backoff(endpoint).await;
        if let Err(err) = handle_stream(client, pool.clone(), sender.clone()).await {
            error!("Closed anime subscription with {err}, Reconnecting in 5 seconds");
            tokio::time::sleep(RECONNECT_INTERVAL).await;
        }
    }
}

async fn connect_with_backoff(endpoint: &str) -> DownloadsClient<tonic::transport::Channel> {
    let mut backoff = BACKOFF_INTERVAL;

    loop {
        match DownloadsClient::connect("http://localhost:8000").await {
            Ok(client) => return client.accept_compressed(CompressionEncoding::Gzip),
            Err(err) => {
                error!(
                    "Failed to connect to {} with error: {}. Retrying in {:.2} seconds",
                    endpoint,
                    err,
                    backoff.as_secs_f32()
                );
                tokio::time::sleep(backoff).await;
                backoff = min(backoff * 2, MAX_BACKOFF);
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
enum ConnectionError {
    #[error(transparent)]
    Status(#[from] tonic::Status),
}

async fn handle_stream(
    mut client: DownloadsClient<tonic::transport::Channel>,
    pool: Pool<Postgres>,
    sender: Sender<DownloadCollection>,
) -> Result<(), ConnectionError> {
    let mut stream = client.subscribe(()).await?;
    info!("Connected to grpc service");
    loop {
        if let Some(message) = stream.get_mut().message().await? {
            debug!("Got message: {message:?}");
            let pool = pool.clone();
            let sender = sender.clone();

            if let Err(err) = send_message(pool, sender, message).await {
                error!("Failed to process incoming message: {err}");
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
enum SendError {
    #[error(transparent)]
    Internal(#[from] Error),
    #[error(transparent)]
    Sender(#[from] tokio::sync::mpsc::error::SendError<DownloadCollection>),
}

async fn send_message(
    pool: Pool<Postgres>,
    sender: Sender<DownloadCollection>,
    message: proto::api::v1::DownloadCollection,
) -> Result<(), SendError> {
    let mut collection: DownloadCollection = message.try_into()?;
    let title = &collection.episode.title;
    let channels: Vec<_> = sqlx::query_file!("queries/find_subscribed_channels.sql", title)
        .fetch(&pool)
        .err_into::<Error>()
        .and_then(|record| async move {
            Ok(Subscriber::Channel {
                channel_id: record.channel_id.parse()?,
                guild_id: record.guild_id.parse()?,
            })
        })
        .try_collect()
        .await?;

    if channels.is_empty() {
        return Ok(());
    }
    collection.subscribers = channels;
    sender.send(collection).await?;
    Ok(())
}

impl TryFrom<proto::api::v1::Episode> for Episode {
    type Error = Error;

    fn try_from(value: proto::api::v1::Episode) -> Result<Self, Self::Error> {
        let created_at_timestamp = value.created_at.ok_or(Error::MissingField("created_at"))?;
        let update_at_timestamp = value.updated_at.ok_or(Error::MissingField("updated_at"))?;
        Ok(Episode {
            created_at: from_timestamp(created_at_timestamp)?,
            updated_at: from_timestamp(update_at_timestamp)?,
            title: value.title,
            number: value.number,
            decimal: Some(value.decimal).filter(|&d| d > 0),
            version: Some(value.version).filter(|&d| d > 0),
            extra: Some(value.extra).filter(|d| !d.is_empty()),
        })
    }
}

impl TryFrom<proto::api::v1::Download> for Download {
    type Error = Error;

    fn try_from(value: proto::api::v1::Download) -> Result<Self, Self::Error> {
        let published_date_timestamp = value
            .published_date
            .ok_or(Error::MissingField("published_date"))?;
        Ok(Download {
            published_date: from_timestamp(published_date_timestamp)?,
            resolution: value.resolution,
            comments: value.comments,
            torrent: value.torrent,
            file_name: value.file_name,
        })
    }
}

impl TryFrom<proto::api::v1::DownloadCollection> for DownloadCollection {
    type Error = Error;

    fn try_from(value: proto::api::v1::DownloadCollection) -> Result<Self, Self::Error> {
        Ok(DownloadCollection {
            episode: value
                .episode
                .ok_or(Error::MissingField("episode"))?
                .try_into()?,
            downloads: value
                .downloads
                .into_iter()
                .map(Download::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            subscribers: vec![],
        })
    }
}

#[allow(clippy::cast_sign_loss)]
fn from_timestamp(timestamp: Timestamp) -> Result<DateTime<Utc>, Error> {
    DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
        .ok_or(Error::InvalidTimeStamp(timestamp))
}
