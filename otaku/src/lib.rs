use std::cmp::min;
use std::fmt::Display;
use std::num::{NonZeroU64, ParseIntError};
use std::time::Duration;

use futures_util::TryStreamExt;
use prost_types::Timestamp;
use sqlx::pool::Pool;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::Postgres;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Sender;
use tonic::codec::CompressionEncoding;
use tracing::{debug, error, info, instrument};

use proto::api::v1::downloads_client::DownloadsClient;

pub mod db;

const MAX_BACKOFF: Duration = Duration::from_secs(30);
const BACKOFF_INTERVAL: Duration = Duration::from_millis(125);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Subscriptions(#[from] SubscriptionError),
    #[error(transparent)]
    FromGrpc(#[from] ConversionError),
    #[error(transparent)]
    Sender(#[from] SendError<DownloadCollection>),
}

#[derive(thiserror::Error, Debug)]
enum ConnectionError {
    #[error(transparent)]
    Status(#[from] tonic::Status),
    #[error("The connection was closed by the remote")]
    Closed,
}

#[derive(thiserror::Error, Debug)]
pub enum ConversionError {
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Encounter invalid timestamp")]
    InvalidTimeStamp(Timestamp),
}

#[derive(thiserror::Error, Debug)]
pub enum SubscriptionError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("{0} for {1}")]
    ParseInt(#[source] ParseIntError, &'static str),
    #[error("Found no subscriptions")]
    Empty,
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

pub async fn subscribe(
    endpoint: &'static str,
    pool: Pool<Postgres>,
    sender: Sender<Subscribed<DownloadCollection>>,
) {
    loop {
        let client = connect_with_backoff(endpoint).await;
        if let Err(err) = handle_stream(client, pool.clone(), sender.clone()).await {
            error!("Closed anime subscription with {err}, Reconnecting in 5 seconds");
            tokio::time::sleep(RECONNECT_INTERVAL).await;
        }
    }
}

async fn connect_with_backoff(
    endpoint: &'static str,
) -> DownloadsClient<tonic::transport::Channel> {
    let mut backoff = BACKOFF_INTERVAL;

    loop {
        match DownloadsClient::connect(endpoint).await {
            Ok(client) => return client.accept_compressed(CompressionEncoding::Gzip),
            Err(err) => {
                error!(
                    "Failed to connect to {endpoint} with error: {err}. Retrying in {:.2} seconds",
                    backoff.as_secs_f32()
                );
                tokio::time::sleep(backoff).await;
                backoff = min(backoff * 2, MAX_BACKOFF);
            }
        }
    }
}

async fn handle_stream(
    mut client: DownloadsClient<tonic::transport::Channel>,
    pool: Pool<Postgres>,
    sender: Sender<Subscribed<DownloadCollection>>,
) -> Result<(), ConnectionError> {
    let mut stream = client.subscribe(()).await?;
    info!("Connected to grpc service");
    loop {
        let Some(incoming_message) = stream.get_mut().message().await? else {
            return Err(ConnectionError::Closed);
        };
        process_message(pool.clone(), sender.clone(), incoming_message).await;
    }
}

#[instrument(skip_all)]
async fn process_message(
    pool: Pool<Postgres>,
    sender: Sender<Subscribed<DownloadCollection>>,
    incoming_message: proto::api::v1::DownloadCollection,
) {
    debug!("Got message: {incoming_message:?}");

    // Filter incomplete messages
    if !incoming_message
        .downloads
        .iter()
        .any(|download| download.resolution == "1080p")
    {
        debug!("Message was incomplete, skipping");
        return;
    }

    let collection: DownloadCollection = match incoming_message.try_into() {
        Ok(collection) => collection,
        Err(err) => {
            error!("Failed to convert message to DownloadCollection: {err}");
            return;
        }
    };

    let Ok(subscribers) = get_subscribers(pool, &collection.episode.title).await else {
        return;
    };

    let outbound_message = Subscribed {
        content: collection,
        subscribers,
    };
    if let Err(err) = sender.send(outbound_message).await {
        error!("Failed to forward incoming message: {err}");
    }
}

#[instrument(skip(pool))]
async fn get_subscribers(
    pool: Pool<Postgres>,
    title: &str,
) -> Result<Vec<Subscriber>, SubscriptionError> {
    let channels: Vec<_> = sqlx::query_file!("queries/find_subscribed_channels.sql", title)
        .fetch(&pool)
        .err_into::<SubscriptionError>()
        .and_then(|record| async move {
            Ok(Subscriber::Channel {
                channel_id: record
                    .channel_id
                    .parse()
                    .map_err(|err| SubscriptionError::ParseInt(err, "channel_id"))?,
                guild_id: record
                    .guild_id
                    .parse()
                    .map_err(|err| SubscriptionError::ParseInt(err, "guild_id"))?,
            })
        })
        .try_collect()
        .await?;

    if channels.is_empty() {
        let error = SubscriptionError::Empty;
        info!("{error}");
        return Err(error);
    }
    Ok(channels)
}

impl TryFrom<proto::api::v1::Episode> for Episode {
    type Error = ConversionError;

    fn try_from(value: proto::api::v1::Episode) -> Result<Self, Self::Error> {
        let created_at_timestamp = value
            .created_at
            .ok_or(ConversionError::MissingField("created_at"))?;
        let update_at_timestamp = value
            .updated_at
            .ok_or(ConversionError::MissingField("updated_at"))?;
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
    type Error = ConversionError;

    fn try_from(value: proto::api::v1::Download) -> Result<Self, Self::Error> {
        let published_date_timestamp = value
            .published_date
            .ok_or(ConversionError::MissingField("published_date"))?;
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
    type Error = ConversionError;

    fn try_from(value: proto::api::v1::DownloadCollection) -> Result<Self, Self::Error> {
        Ok(DownloadCollection {
            episode: value
                .episode
                .ok_or(ConversionError::MissingField("episode"))?
                .try_into()?,
            downloads: value
                .downloads
                .into_iter()
                .map(Download::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[allow(clippy::cast_sign_loss)]
fn from_timestamp(timestamp: Timestamp) -> Result<DateTime<Utc>, ConversionError> {
    DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
        .ok_or(ConversionError::InvalidTimeStamp(timestamp))
}
