use const_fnv1a_hash::fnv1a_hash_str_128;
use const_format::formatc;
use itertools::Itertools;
use std::borrow::Cow;
use url::Url;

use error::Error;

use crate::models::{ContentFilter, Format, Gif, Response};

pub mod error;
pub mod models;

// TODO clean this up once more things are stable
static DEFAULT_CUSTOMER_ID: &str = formatc!(
    "{:X}",
    fnv1a_hash_str_128(match option_env!("CARGO_PKG_NAME") {
        Some(name) => name,
        None => env!("CARGO_PKG_NAME"),
    })
);

#[derive(Debug, Clone)]
pub struct Klipy<'config> {
    api_key: &'config str,
    customer_id: &'config str,
    reqwest: reqwest::Client,
    base_config: Option<Config<'config>>,
}

impl<'config> Klipy<'config> {
    #[must_use]
    pub fn new(api_key: &'config str) -> Klipy<'config> {
        Self::with_config(api_key, None, None)
    }

    #[must_use]
    pub fn with_customer_id(api_key: &'config str, customer_id: &'config str) -> Klipy<'config> {
        Self::with_config(api_key, Some(customer_id), None)
    }

    #[must_use]
    pub fn with_config(
        api_key: &'config str,
        customer_id: Option<&'config str>,
        config: Option<Config<'config>>,
    ) -> Klipy<'config> {
        let customer_id = customer_id.unwrap_or(DEFAULT_CUSTOMER_ID);
        Klipy {
            api_key,
            customer_id,
            reqwest: reqwest::Client::new(),
            base_config: config,
        }
    }

    fn build_query<'a: 'config>(
        &'a self,
        query: &'a str,
        config: Option<Config<'a>>,
    ) -> Vec<(&'static str, Cow<'config, str>)> {
        match self.merge_config(config) {
            None => vec![
                ("q", Cow::Borrowed(query)),
                ("customer_id", Cow::Borrowed(self.customer_id)),
            ],

            Some(cfg) => {
                // always overallocate to maximum capacity
                let mut params: Vec<(&str, Cow<'_, str>)> = Vec::with_capacity(7);
                params.push(("q", Cow::Borrowed(query)));
                params.push(("customer_id", Cow::Borrowed(self.customer_id)));
                if let Some(locale) = cfg.locale {
                    params.push(("locale", Cow::Borrowed(locale)));
                }
                if let Some(content_filter) = cfg.content_filter {
                    let filter = content_filter.into();
                    params.push(("content_filter", filter));
                }
                if let Some(format_filter) = cfg.format_filter {
                    let filter = format_filter
                        .iter()
                        .map(Into::<&'static str>::into)
                        .join(",");
                    params.push(("format_filter", Cow::Owned(filter)));
                }
                if let Some(per_page) = cfg.per_page {
                    params.push(("per_page", Cow::Owned(per_page.to_string())));
                }
                if let Some(page) = cfg.page {
                    params.push(("page", Cow::Owned(page.to_string())));
                }
                params
            }
        }
    }

    /// Search for GIFs with the given query.
    ///
    /// # Errors
    ///
    /// Returns an error when klipy cannot be reached or an error is returned from the api.
    pub async fn search(&self, query: &str, config: Option<Config<'_>>) -> Result<Vec<Gif>, Error> {
        let query = self.build_query(query, config);
        let url = format!("https://api.klipy.com/api/v1/{}/gifs/search", self.api_key);
        let url = Url::parse_with_params(&url, &query)?;
        let result: Response<Gif> = self.reqwest.get(url).send().await?.json().await?;
        Ok(result.data.data)
    }

    fn merge_config<'a: 'config>(&self, config: Option<Config<'a>>) -> Option<Config<'config>> {
        match (self.base_config, config) {
            (None, None) => None,
            (cfg, None) | (None, cfg) => cfg,
            (Some(base_cfg), Some(other)) => base_cfg.merge(other),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config<'config> {
    /// Strongly recommended
    locale: Option<&'config str>,
    /// Strongly recommended
    content_filter: Option<ContentFilter>,
    /// Strongly recommended
    format_filter: Option<&'config [Format]>,
    per_page: Option<u8>,
    page: Option<u32>,
}

impl<'config> Config<'config> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            locale: None,
            content_filter: None,
            format_filter: None,
            per_page: None,
            page: None,
        }
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
    pub const fn format_filter(mut self, format_filter: &'config [Format]) -> Self {
        self.format_filter = Some(format_filter);
        self
    }

    #[must_use]
    pub const fn per_page(mut self, limit: u8) -> Self {
        self.per_page = Some(limit);
        self
    }

    #[must_use]
    pub const fn page(mut self, position: u32) -> Self {
        self.page = Some(position);
        self
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Option<Self> {
        if let Some(locale) = other.locale {
            self.locale.replace(locale);
        }
        if let Some(content_filter) = other.content_filter {
            self.content_filter.replace(content_filter);
        }
        if let Some(format_filter) = other.format_filter {
            self.format_filter.replace(format_filter);
        }
        if let Some(limit) = other.per_page {
            self.per_page.replace(limit);
        }
        if let Some(position) = other.page {
            self.page.replace(position);
        }
        Some(self)
    }
}

impl Default for Config<'static> {
    fn default() -> Self {
        Self::new()
    }
}
