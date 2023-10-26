#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools;
use url::Url;

use error::Error;

use crate::models::{ContentFilter, Gif, MediaFilter, Response};

pub mod error;
pub mod models;

#[derive(Debug, Clone)]
pub struct Client {
    api_key: Arc<str>,
    reqwest: reqwest::Client,
    base_config: Option<Config>,
}

impl Client {
    #[must_use]
    pub fn new(api_key: String) -> Client {
        Self::with_config(api_key, None)
    }

    #[must_use]
    pub fn with_config(api_key: String, config: Option<Config>) -> Client {
        Client {
            api_key: api_key.into(),
            reqwest: reqwest::Client::new(),
            base_config: config,
        }
    }

    fn build_query_string<'a>(
        &'a self,
        query: &'a str,
        config: Option<&'a Config>,
    ) -> Vec<(&'static str, Cow<'a, str>)> {
        // always overallocate to maximum capacity
        let mut params = Vec::with_capacity(9);
        params.push(("key", self.api_key.as_ref().into()));
        params.push(("q", query.into()));
        if let Some(country) = config
            .and_then(|c| c.country.as_ref())
            .or(self.base_config.as_ref().and_then(|c| c.country.as_ref()))
        {
            params.push(("country", country.into()));
        }
        if let Some(locale) = config
            .and_then(|c| c.locale.as_ref())
            .or(self.base_config.as_ref().and_then(|c| c.locale.as_ref()))
        {
            params.push(("locale", locale.into()));
        }
        if let Some(content_filter) = config.and_then(|c| c.content_filter.as_ref()).or(self
            .base_config
            .as_ref()
            .and_then(|c| c.content_filter.as_ref()))
        {
            let filter = Into::<&'static str>::into(content_filter).into();
            params.push(("contentfilter", filter));
        }
        if let Some(media_filter) = config.and_then(|c| c.media_filter.as_ref()).or(self
            .base_config
            .as_ref()
            .and_then(|c| c.media_filter.as_ref()))
        {
            let filter = media_filter
                .iter()
                .map(Into::<&'static str>::into)
                .join(",");
            params.push(("media_filter", filter.into()));
        }
        if let Some(random) = config
            .and_then(|c| c.random.as_ref())
            .or(self.base_config.as_ref().and_then(|c| c.random.as_ref()))
        {
            params.push(("random", random.to_string().into()));
        }
        if let Some(limit) = config
            .and_then(|c| c.limit.as_ref())
            .or(self.base_config.as_ref().and_then(|c| c.limit.as_ref()))
        {
            params.push(("limit", limit.to_string().into()));
        }
        if let Some(position) = config
            .and_then(|c| c.position.as_ref())
            .or(self.base_config.as_ref().and_then(|c| c.position.as_ref()))
        {
            params.push(("pos", position.into()));
        }
        params
    }

    /// Search for gifs with the given query.
    ///
    /// # Errors
    ///
    /// Returns an error when tenor cannot be reached or an error is returned from the api.
    pub async fn search(&self, query: &str, config: Option<&Config>) -> Result<Vec<Gif>, Error> {
        let query = self.build_query_string(query, config);

        let url = Url::parse_with_params("https://tenor.googleapis.com/v2/search", &query)?;
        let result: Response<Vec<Gif>> = self.reqwest.get(url).send().await?.json().await?;
        Ok(result.results)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Strongly recommended
    country: Option<String>,
    /// Strongly recommended
    locale: Option<String>,
    /// Strongly recommended
    content_filter: Option<ContentFilter>,
    /// Strongly recommended
    media_filter: Option<Vec<MediaFilter>>,
    random: Option<bool>,
    limit: Option<u8>,
    position: Option<String>,
}

impl Config {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn country(mut self, country: String) -> Self {
        self.country = Some(country);
        self
    }

    #[must_use]
    pub fn locale(mut self, locale: String) -> Self {
        self.locale = Some(locale);
        self
    }

    #[must_use]
    pub fn content_filter(mut self, content_filter: ContentFilter) -> Self {
        self.content_filter = Some(content_filter);
        self
    }

    #[must_use]
    pub fn media_filter(mut self, media_filter: Vec<MediaFilter>) -> Self {
        self.media_filter = Some(media_filter);
        self
    }

    #[must_use]
    pub fn random(mut self, random: bool) -> Self {
        self.random = Some(random);
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn position(mut self, position: String) -> Self {
        self.position = Some(position);
        self
    }
}
