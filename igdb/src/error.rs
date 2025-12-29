use reqwest::header::InvalidHeaderValue;

#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("Reqwest could not be initialized: {0}")]
    ReqwestBuilder(#[from] reqwest::Error),
    #[error("Invalid Client ID: {0}")]
    InvalidClientId(#[from] InvalidHeaderValue),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Failed to parse response: {0}")]
    DeserializeJson(#[from] serde_json::Error),
    #[error("Failed to parse url: {0}")]
    Url(#[from] url::ParseError),
}
