#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Failed to parse response: {0}")]
    DeserializeJson(#[from] serde_json::Error),
    #[error("Failed to parse response: {0}")]
    Url(#[from] url::ParseError),
}
