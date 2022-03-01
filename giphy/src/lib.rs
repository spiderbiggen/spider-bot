#[macro_use]
extern crate serde;

use reqwest::Client as ReqClient;
use thiserror::Error as ThisError;
use url::Url;

use crate::models::*;

pub mod models;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    ParseUrl(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

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
