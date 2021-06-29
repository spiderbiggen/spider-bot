#[macro_use]
extern crate serde;

use std::error::Error;

use reqwest::Client as ReqClient;
use url::Url;

use crate::models::*;

pub mod models;

pub struct Client {
    pub api_key: String,
    pub reqwest: ReqClient,
    pub content_filter: ContentFilter,
}

impl Client {
    pub fn new<S: Into<String>>(api_key: S, content_filter: Option<ContentFilter>) -> Client {
        Client {
            api_key: api_key.into(),
            reqwest: ReqClient::new(),
            content_filter: content_filter.unwrap_or(ContentFilter::High),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Gif>, Box<dyn Error>> {
        let url = Url::parse_with_params(
            "https://g.tenor.com/v1/search",
            &[
                ("key", self.api_key.as_str()),
                ("q", query),
                ("locale", "en"),
                ("contentfilter", self.content_filter.into()),
                ("media_filter", MediaFilter::Minimal.into()),
            ],
        )?;

        let result: Response<Vec<Gif>> = self.reqwest.get(url).send().await?.json().await?;
        Ok(result.results)
    }

    pub async fn random(&self, query: &str) -> Result<Vec<Gif>, Box<dyn Error>> {
        let url = Url::parse_with_params(
            "https://g.tenor.com/v1/random",
            &[
                ("key", self.api_key.as_str()),
                ("q", query),
                ("locale", "en"),
                ("contentfilter", self.content_filter.into()),
                ("media_filter", MediaFilter::Minimal.into()),
                ("limit", "50"),
            ],
        )?;

        let result: Response<Vec<Gif>> = self.reqwest.get(url).send().await?.json().await?;
        Ok(result.results)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
