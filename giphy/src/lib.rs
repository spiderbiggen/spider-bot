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
}

impl Client {
    pub fn new(api_key: String) -> Client {
        Client {
            api_key,
            reqwest: ReqClient::new(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error>> {
        let url = Url::parse_with_params(
            "https://api.giphy.com/v1/gifs/search?limit=25&offset=0&rating=g&lang=en",
            &[("api_key", self.api_key.as_str()), ("q", query)],
        )?;

        let result: Results<SearchResult> = self.reqwest.get(url)
            .send().await?
            .json().await?;
        return Ok(result.data);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
