mod dto;

use db::{BotDatabase, DatabaseConnection};
use domain::{DownloadCollection, Subscribed};
use eventsource_stream::{EventStreamError, Eventsource};
use futures::{Stream, StreamExt};
use reqwest::header::{ACCEPT, ACCEPT_ENCODING};
use std::cmp::min;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tracing::instrument;

const MAX_BACKOFF: Duration = Duration::from_secs(30);
const BACKOFF_INTERVAL: Duration = Duration::from_millis(125);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Keep-alives arrive roughly every 15 seconds, so a silent connection is dead after this.
const READ_TIMEOUT: Duration = Duration::from_secs(45);
const UPDATES_PATH: &str = "/v1/downloads/updates";
const DOWNLOAD_EVENT: &str = "download";

type StreamError = EventStreamError<reqwest::Error>;

#[derive(thiserror::Error, Debug)]
enum ConnectError {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("unexpected status {0}")]
    Status(reqwest::StatusCode),
}

pub async fn subscribe(
    endpoint: &'static str,
    db: BotDatabase,
    sender: Sender<Subscribed<DownloadCollection>>,
) {
    // No overall request timeout: it would also kill the long-lived stream.
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .expect("Failed to build the HTTP client");
    let url = format!("{}{UPDATES_PATH}", endpoint.trim_end_matches('/'));

    loop {
        let response = connect_with_backoff(&client, &url).await;
        let result = handle_stream(response, &db, &sender).await;

        match result {
            Ok(()) => tracing::error!("Anime subscription dropped"),
            Err(err) => tracing::error!("Anime subscription dropped: {err}"),
        }

        tokio::time::sleep(RECONNECT_INTERVAL).await;
    }
}

async fn connect(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, ConnectError> {
    let response = client
        .get(url)
        .header(ACCEPT, "text/event-stream")
        // Compression would buffer events, so ask for the stream as-is.
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        Err(ConnectError::Status(status))
    }
}

async fn connect_with_backoff(client: &reqwest::Client, url: &str) -> reqwest::Response {
    let mut backoff = BACKOFF_INTERVAL;

    loop {
        match connect(client, url).await {
            Ok(response) => return response,
            Err(err) => {
                tracing::error!(
                    "Failed to connect to {url} with error: {err}. Retrying in {:.2} seconds",
                    backoff.as_secs_f32()
                );
                tokio::time::sleep(backoff).await;
                backoff = min(backoff * 2, MAX_BACKOFF);
            }
        }
    }
}

/// Decodes the `download` events of an SSE response. Other events and keep-alive comments are
/// ignored, and events with an undecodable payload are logged and skipped.
fn download_events(
    response: reqwest::Response,
) -> impl Stream<Item = Result<dto::DownloadCollection, StreamError>> {
    response
        .bytes_stream()
        .eventsource()
        .filter_map(|event| async move {
            match event {
                Ok(event) if event.event == DOWNLOAD_EVENT => {
                    match serde_json::from_str(&event.data) {
                        Ok(message) => Some(Ok(message)),
                        Err(err) => {
                            tracing::error!("Failed to decode an incoming message: {err}");
                            None
                        }
                    }
                }
                Ok(event) => {
                    tracing::debug!("Ignoring event of type {:?}", event.event);
                    None
                }
                Err(err) => Some(Err(err)),
            }
        })
}

async fn handle_stream(
    response: reqwest::Response,
    db: &BotDatabase,
    sender: &Sender<Subscribed<DownloadCollection>>,
) -> Result<(), StreamError> {
    tracing::info!("Connected to anime service");

    let mut stream = std::pin::pin!(download_events(response));
    while let Some(message) = stream.next().await {
        process_message(db, sender, message?).await;
    }
    Ok(())
}

#[instrument(skip_all)]
async fn process_message(
    db: &BotDatabase,
    sender: &Sender<Subscribed<DownloadCollection>>,
    incoming_message: dto::DownloadCollection,
) {
    tracing::debug!("Got message: {incoming_message:?}");

    // Filter incomplete messages
    if !incoming_message
        .downloads
        .iter()
        .any(|download| download.resolution == 1080)
    {
        tracing::debug!("Message was incomplete, skipping");
        return;
    }

    let collection: DownloadCollection = match incoming_message.try_into() {
        Ok(collection) => collection,
        Err(err) => {
            tracing::error!("Failed to convert an incoming message to DownloadCollection: {err}");
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
        tracing::error!("Failed to forward an incoming message: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const EPISODE: &str = r#"{"title":"Show","variant":"episode","episode":7,"created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":[{"comments":"c","resolution":1080,"torrent":"t","file_name":"f","published_date":"2026-11-20T16:02:28Z"}]}"#;
    const MOVIE: &str = r#"{"title":"Film","variant":"movie","future":true,"created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":[]}"#;

    /// Serves one SSE response, writing each chunk separately with a short pause in between.
    async fn serve(status: &str, chunks: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let head = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n"
        );
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            socket.write_all(head.as_bytes()).await.unwrap();
            for chunk in chunks {
                socket.write_all(chunk.as_bytes()).await.unwrap();
                socket.flush().await.unwrap();
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            socket.shutdown().await.unwrap();
        });
        url
    }

    #[tokio::test]
    async fn decodes_download_events() {
        let (first, second) = EPISODE.split_at(40);
        let chunks = vec![
            ": keep-alive\n\n".to_string(),
            format!("event: download\ndata: {first}"),
            format!("{second}\n\n"),
            ": keep-alive\n\n".to_string(),
            "event: other\ndata: {}\n\n".to_string(),
            "event: download\ndata: not json\n\n".to_string(),
            format!("event: download\ndata: {MOVIE}\n\n"),
        ];
        let url = serve("200 OK", chunks).await;

        let client = reqwest::Client::new();
        let response = connect(&client, &format!("{url}{UPDATES_PATH}"))
            .await
            .unwrap();
        let messages: Vec<_> = download_events(response)
            .map(|message| domain::DownloadCollection::try_from(message.unwrap()).unwrap())
            .collect()
            .await;

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].title, "Show");
        assert_eq!(
            messages[0].variant,
            domain::DownloadVariant::Episode(domain::Episode {
                number: 7,
                decimal: None,
                version: None,
                extra: None,
            })
        );
        assert_eq!(messages[0].downloads[0].resolution, 1080);
        assert_eq!(messages[1].title, "Film");
        assert_eq!(messages[1].variant, domain::DownloadVariant::Movie);
    }

    #[tokio::test]
    async fn non_success_status_is_a_failed_connection() {
        let url = serve("503 Service Unavailable", vec![]).await;
        let client = reqwest::Client::new();
        let err = connect(&client, &url).await.unwrap_err();
        assert!(matches!(
            err,
            ConnectError::Status(reqwest::StatusCode::SERVICE_UNAVAILABLE)
        ));
    }
}
