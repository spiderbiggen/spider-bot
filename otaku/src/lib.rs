use db::{BotDatabase, DatabaseConnection};
use domain::{DownloadCollection, Subscribed};
use proto::api::v2::downloads_client::DownloadsClient;
use std::cmp::min;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::SendError;
use tonic::codec::CompressionEncoding;
use tracing::{debug, error, info, instrument};

const MAX_BACKOFF: Duration = Duration::from_secs(30);
const BACKOFF_INTERVAL: Duration = Duration::from_millis(125);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    FromDB(#[from] db::Error),
    #[error(transparent)]
    FromGrpc(#[from] proto::Error),
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

pub async fn subscribe(
    endpoint: &'static str,
    db: BotDatabase,
    sender: Sender<Subscribed<DownloadCollection>>,
) {
    loop {
        let client = connect_with_backoff(endpoint).await;
        if let Err(err) = handle_stream(client, &db, &sender).await {
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
    db: &BotDatabase,
    sender: &Sender<Subscribed<DownloadCollection>>,
) -> Result<(), ConnectionError> {
    let mut stream = client.subscribe(()).await?;
    info!("Connected to grpc service");
    loop {
        let Some(incoming_message) = stream.get_mut().message().await? else {
            return Err(ConnectionError::Closed);
        };
        process_message(db, sender, incoming_message).await;
    }
}

#[instrument(skip_all)]
async fn process_message(
    db: &BotDatabase,
    sender: &Sender<Subscribed<DownloadCollection>>,
    incoming_message: proto::api::v2::DownloadCollection,
) {
    debug!("Got message: {incoming_message:?}");

    // Filter incomplete messages
    if !incoming_message
        .downloads
        .iter()
        .any(|download| download.resolution == 1080)
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

    let Ok(Some(subscribers)) = db.get_subscribers(&collection.title).await else {
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
