use serde::Deserialize;

use reqwest::Client as ReqClient;
use strum_macros::{EnumString, IntoStaticStr};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    ParseUrl(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub data: T,
}

#[derive(Deserialize, Debug)]
pub struct Gif {
    pub id: String,
    pub url: String,
    pub title: String,
    pub embed_url: String,
    pub rating: String,
}

#[derive(Debug, Copy, Clone, PartialEq, EnumString, IntoStaticStr)]
pub enum ContentFilter {
    #[strum(serialize = "g")]
    High,
    #[strum(serialize = "pg")]
    Medium,
    #[strum(serialize = "pg13")]
    Low,
    #[strum(serialize = "r")]
    Off,
}

pub struct Client {
    pub api_key: String,
    pub reqwest: ReqClient,
    pub content_filter: ContentFilter,
}

impl Client {
    pub fn new<S: Into<String>>(api_key: S, content_filter: Option<ContentFilter>) -> Self {
        Client {
            api_key: api_key.into(),
            reqwest: ReqClient::new(),
            content_filter: content_filter.unwrap_or(ContentFilter::High),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Gif>> {
        let url = Url::parse_with_params(
            "https://api.giphy.com/v1/gifs/search",
            &[
                // limit
                // offset
                ("api_key", self.api_key.as_str()),
                ("q", query),
                ("lang", "en"),
                ("rating", self.content_filter.into()),
            ],
        )?;

        let result: Response<Vec<Gif>> = self.reqwest.get(url).send().await?.json().await?;
        return Ok(result.data);
    }

    pub async fn random(&self, tag: &str) -> Result<Gif> {
        let url = Url::parse_with_params(
            "https://api.giphy.com/v1/gifs/random?rating=g",
            &[
                ("api_key", self.api_key.as_str()),
                ("tag", tag),
                ("lang", "en"),
                ("rating", self.content_filter.into()),
            ],
        )?;

        let result: Response<Gif> = self.reqwest.get(url).send().await?.json().await?;
        return Ok(result.data);
    }
}
