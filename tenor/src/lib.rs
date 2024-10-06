use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools;
use url::Url;

use error::Error;

use crate::models::{ContentFilter, Gif, MediaFilter, Response};

pub mod error;
pub mod models;

#[derive(Debug, Clone)]
pub struct Client<'config> {
    api_key: Arc<str>,
    reqwest: reqwest::Client,
    base_config: Option<Config<'config>>,
}

impl<'config> Client<'config> {
    #[must_use]
    pub fn new(api_key: String) -> Client<'config> {
        Self::with_config(api_key, None)
    }

    #[must_use]
    pub fn with_config(api_key: String, config: Option<Config<'config>>) -> Client<'config> {
        Client {
            api_key: api_key.into(),
            reqwest: reqwest::Client::new(),
            base_config: config,
        }
    }

    fn build_query_string<'a: 'config>(
        &'a self,
        query: &'a str,
        config: Option<Config<'a>>,
    ) -> Vec<(&'static str, Cow<'config, str>)> {
        // always overallocate to maximum capacity
        match self.merge_config(config) {
            None => vec![
                ("key", self.api_key.as_ref().into()),
                ("q", Cow::Borrowed(query)),
            ],

            Some(merged_config) => {
                let mut params: Vec<(&str, Cow<'_, str>)> = Vec::with_capacity(9);
                params.push(("key", self.api_key.as_ref().into()));
                params.push(("q", Cow::Borrowed(query)));
                if let Some(country) = merged_config.country {
                    params.push(("country", Cow::Borrowed(country)));
                }
                if let Some(locale) = merged_config.locale {
                    params.push(("locale", Cow::Borrowed(locale)));
                }
                if let Some(content_filter) = merged_config.content_filter {
                    let filter = content_filter.into();
                    params.push(("contentfilter", filter));
                }
                if let Some(media_filter) = merged_config.media_filter {
                    let filter = media_filter
                        .iter()
                        .map(Into::<&'static str>::into)
                        .join(",");
                    params.push(("media_filter", Cow::Owned(filter)));
                }
                if let Some(random) = merged_config.random {
                    let random = if random { "true" } else { "false" };
                    params.push(("random", Cow::Borrowed(random)));
                }
                if let Some(limit) = merged_config.limit {
                    params.push(("limit", limit.to_string().into()));
                }
                if let Some(position) = merged_config.position {
                    params.push(("pos", Cow::Borrowed(position)));
                }
                params
            }
        }
    }

    /// Search for GIFs with the given query.
    ///
    /// # Errors
    ///
    /// Returns an error when tenor cannot be reached or an error is returned from the api.
    pub async fn search(&self, query: &str, config: Option<Config<'_>>) -> Result<Vec<Gif>, Error> {
        let query = self.build_query_string(query, config);

        let url = Url::parse_with_params("https://tenor.googleapis.com/v2/search", &query)?;
        let result: Response<Vec<Gif>> = self.reqwest.get(url).send().await?.json().await?;
        Ok(result.results)
    }

    fn merge_config<'a: 'config>(&self, config: Option<Config<'a>>) -> Option<Config<'config>> {
        match (self.base_config, config) {
            (None, None) => None,
            (Some(base_cfg), None) => Some(base_cfg),
            (None, Some(cfg)) => Some(cfg),
            (Some(base_cfg), Some(mut cfg)) => {
                cfg.country = cfg.country.or(base_cfg.country);
                cfg.locale = cfg.locale.or(base_cfg.locale);
                cfg.content_filter = cfg.content_filter.or(base_cfg.content_filter);
                cfg.media_filter = cfg.media_filter.or(base_cfg.media_filter);
                cfg.random = cfg.random.or(base_cfg.random);
                cfg.limit = cfg.limit.or(base_cfg.limit);
                Some(cfg)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config<'config> {
    /// Strongly recommended
    country: Option<&'config str>,
    /// Strongly recommended
    locale: Option<&'config str>,
    /// Strongly recommended
    content_filter: Option<ContentFilter>,
    /// Strongly recommended
    media_filter: Option<&'config [MediaFilter]>,
    random: Option<bool>,
    limit: Option<u8>,
    position: Option<&'config str>,
}

impl<'config> Config<'config> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            country: None,
            locale: None,
            content_filter: None,
            media_filter: None,
            random: None,
            limit: None,
            position: None,
        }
    }

    #[must_use]
    pub const fn country(mut self, country: &'config str) -> Self {
        self.country = Some(country);
        self
    }

    #[must_use]
    pub const fn locale(mut self, country: &'config str) -> Self {
        self.locale = Some(country);
        self
    }

    #[must_use]
    pub const fn content_filter(mut self, content_filter: ContentFilter) -> Self {
        self.content_filter = Some(content_filter);
        self
    }

    #[must_use]
    pub const fn media_filter(mut self, media_filter: &'config [MediaFilter]) -> Self {
        self.media_filter = Some(media_filter);
        self
    }

    #[must_use]
    pub const fn random(mut self, random: bool) -> Self {
        self.random = Some(random);
        self
    }

    #[must_use]
    pub const fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub const fn position(mut self, position: &'config str) -> Self {
        self.position = Some(position);
        self
    }
}

impl Default for Config<'static> {
    fn default() -> Self {
        Self::new()
    }
}
