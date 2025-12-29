pub mod error;
pub mod models;

use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use reqwest::Method;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::borrow::Cow;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{BuilderError, Error};
use crate::models::{AccessToken, Game, GameType};

/// 4 requests per second
pub const IGDB_RATE_LIMIT: Quota =
    Quota::per_second(NonZeroU32::new(4).expect("4 requests per second is a valid rate limit"));

#[derive(Clone)]
pub struct Igdb<'config> {
    client_id: &'config str,
    client_secret: &'config str,
    client: reqwest::Client,
    governor: Arc<DefaultDirectRateLimiter>,
    access_token: Arc<RwLock<Option<AccessToken>>>,
}

impl<'config> Igdb<'config> {
    pub fn new(client_id: &'config str, client_secret: &'config str) -> Result<Self, BuilderError> {
        Self::new_with_governor(
            client_id,
            client_secret,
            Arc::new(RateLimiter::direct(IGDB_RATE_LIMIT)),
        )
    }

    pub fn new_with_governor(
        client_id: &'config str,
        client_secret: &'config str,
        governor: Arc<DefaultDirectRateLimiter>,
    ) -> Result<Self, BuilderError> {
        let mut header_map = reqwest::header::HeaderMap::with_capacity(1);
        header_map.insert("Client-ID", client_id.parse()?);
        let client = reqwest::Client::builder()
            .default_headers(header_map)
            .build()?;

        Ok(Self {
            client_id,
            client_secret,
            client,
            governor,
            access_token: Arc::new(RwLock::new(None)),
        })
    }

    async fn ensure_authenticated(&self) -> Result<Arc<str>, Error> {
        #[derive(Debug, Deserialize)]
        struct AccessTokenResponse {
            access_token: Box<str>,
            expires_in: u64,
        }

        let request_time = Instant::now();
        // Return immediately if we have a valid token and it's not expired yet.
        if let Some(token) = self.access_token.read().await.as_ref()
            && token.expires_at > request_time + Duration::from_secs(5)
        {
            return Ok(Arc::clone(&token.access_token));
        }

        let url = format!(
            "https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials",
            self.client_id, self.client_secret
        );
        let mut write_guard = self.access_token.write().await;
        let request = self.client.post(url);
        let response = request.send().await?.error_for_status()?;
        let body: AccessTokenResponse = response.json().await?;
        let access_token = Arc::from(body.access_token);
        *write_guard = Some(AccessToken {
            access_token: Arc::clone(&access_token),
            expires_at: request_time + Duration::from_secs(body.expires_in),
        });
        Ok(access_token)
    }

    async fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<String>,
    ) -> Result<reqwest::Response, Error> {
        let access_token = self.ensure_authenticated().await?;
        // TODO this should probably have a timeout
        self.governor.until_ready().await;

        let mut request = self.client.request(method, url).bearer_auth(access_token);
        if let Some(body) = body {
            request = request.body(body);
        }
        let result = request.send().await?.error_for_status()?;
        Ok(result)
    }

    async fn post_request(
        &self,
        url: &str,
        body: Option<String>,
    ) -> Result<reqwest::Response, Error> {
        self.request(Method::POST, url, body).await
    }

    pub async fn games(&self, query_builder: Option<&QueryBuilder>) -> Result<Vec<Game>, Error> {
        let url = "https://api.igdb.com/v4/games";
        let body = query_builder.map(QueryBuilder::build);
        let result = self.post_request(url, body).await?;
        let body = result.json().await?;
        Ok(body)
    }

    pub async fn game_types(
        &self,
        query_builder: Option<&QueryBuilder>,
    ) -> Result<Vec<GameType>, Error> {
        let url = "https://api.igdb.com/v4/game_types";
        let body = query_builder.map(QueryBuilder::build);
        let result = self.post_request(url, body).await?;
        let body = result.json().await?;
        Ok(body)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Sort {
    field: &'static str,
    ascending: bool,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    column: Option<&'static str>,
    query: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Limit(NonZeroU16);

impl Limit {
    const _MIN: u16 = NonZeroU16::MIN.get();
    const _MAX: u16 = 500;
    pub const MAX: Self = Limit::new(Self::_MAX).unwrap();
    pub const MIN: Self = Limit::new(Self::_MIN).unwrap();

    /// Creates a new instance of the struct if the provided `limit` is greater than zero and less than or equal to 500.
    ///
    /// # Returns
    /// - `Option<Self>`:
    ///   - Returns `Some(Self)` if the `limit` is non-zero and within the allowable range (1 to 500, inclusive).
    ///   - Returns `None` if the `limit` is zero or exceeds 500.
    ///
    /// # Example
    /// ```
    /// use ::igdb::Limit;
    ///
    /// assert!(Limit::new(0).is_none());    // Invalid case: zero
    /// assert!(Limit::new(1).is_some());  // Valid case
    /// assert!(Limit::new(100).is_some());  // Valid case
    /// assert!(Limit::new(500).is_some());  // Valid case
    /// assert!(Limit::new(501).is_none());  // Invalid case: exceeds limit
    /// ```
    /// ```
    #[must_use]
    pub const fn new(limit: u16) -> Option<Self> {
        match NonZeroU16::new(limit) {
            Some(limit) if limit.get() <= Self::_MAX => Some(Self(limit)),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn get(&self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    fields: FxHashSet<&'static str>,
    exclude: FxHashSet<&'static str>,
    filter: Option<Cow<'static, str>>,
    limit: Option<Limit>,
    offset: Option<NonZeroU32>,
    sort: Vec<Sort>,
    search: Option<SearchQuery>,
}

impl QueryBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn all_fields(mut self) -> Self {
        self.fields.clear();
        self.fields.insert("*");
        self
    }

    #[must_use]
    pub fn fields(mut self, fields: impl IntoIterator<Item = &'static str>) -> Self {
        self.fields.extend(fields);
        self
    }

    #[must_use]
    pub fn exclude(mut self, exclude: impl IntoIterator<Item = &'static str>) -> Self {
        self.exclude.extend(exclude);
        self
    }

    /// # Where clause
    ///
    /// A data filter query, similar to SQL.
    ///
    /// You can use `&` and `|` to join comparators (`AND` and `OR` respectively).
    ///
    /// ## Operators:
    ///
    ///     = equals
    ///     != not equals
    ///     > is larger than
    ///     >= is larger than or equal to
    ///     < is smaller than
    ///     <= is smaller than or equal to
    ///     [] contains all of these values
    ///     ![] does not contain all of these values
    ///     () contains at least one of these values
    ///     !() does not contain any of these values
    ///     {} contains all of these values exclusively
    ///
    /// # Examples
    ///
    /// A simple filter for entries where the id is 55.
    ///
    ///     where id = 55
    /// Instead of specific values, you can also use null, true, and false.
    ///
    ///     where enabled = true
    /// Given the column genres contain a single number, we can filter results with parenthesis () to check if it contains any of those values. If the genre is 1, 2 or 3, it will be returned in the results.
    ///
    ///     where genres = (1,2,3)
    /// Given the column genres is an array of numbers, we can use the in operator [] to ensure all specified values appear in the array. In the example below, if genres does not include 1 and 2 and 3, it will not be matched.
    ///
    ///     where genres = [1,2,3]
    /// Given the column genres is an array of numbers, we can use the exclusive operator {} to ensure only the specified values appear in the array. In the example below, if genres only contains 1 and 2, it will be matched.
    ///
    ///     where genres = {1,2}
    /// As per the example above, when only looking for a single value, you can do the following
    ///
    ///     where genres = 1
    #[must_use]
    pub fn filter(mut self, filter: impl Into<Cow<'static, str>>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: Limit) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = NonZeroU32::new(offset);
        self
    }

    #[must_use]
    pub fn sort(mut self, field: &'static str, ascending: bool) -> Self {
        self.sort.push(Sort { field, ascending });
        self
    }

    #[must_use]
    pub fn sort_asc(mut self, field: &'static str) -> Self {
        self.sort.push(Sort {
            field,
            ascending: true,
        });
        self
    }

    #[must_use]
    pub fn sort_desc(mut self, field: &'static str) -> Self {
        self.sort.push(Sort {
            field,
            ascending: false,
        });
        self
    }

    #[must_use]
    pub fn search(mut self, query: impl Into<String>) -> Self {
        self.search = Some(SearchQuery {
            column: None,
            query: query.into(),
        });
        self
    }

    #[must_use]
    pub fn search_column(mut self, column: &'static str, query: impl Into<String>) -> Self {
        self.search = Some(SearchQuery {
            column: Some(column),
            query: query.into(),
        });
        self
    }

    #[must_use]
    pub fn build(&self) -> String {
        let mut query = String::new();
        if !self.fields.is_empty() {
            query.push_str("fields ");
            for (index, field) in self.fields.iter().enumerate() {
                if index != 0 {
                    query.push(',');
                }
                query.push_str(field);
            }
            query.push(';');
        }
        if !self.exclude.is_empty() {
            query.push_str("exclude ");
            for (index, field) in self.exclude.iter().enumerate() {
                if index != 0 {
                    query.push(',');
                }
                query.push_str(field);
            }
            query.push(';');
        }

        if let Some(filter) = &self.filter {
            let filter = filter.strip_prefix("where ").unwrap_or(filter);
            if !filter.is_empty() {
                query.push_str("where ");
                query.push_str(filter);
                query.push(';');
            }
        }

        if let Some(limit) = self.limit {
            query.push_str("limit ");
            query.push_str(&limit.get().to_string());
            query.push(';');
        }
        if let Some(offset) = self.offset {
            query.push_str("offset ");
            query.push_str(&offset.get().to_string());
            query.push(';');
        }

        if !self.sort.is_empty() {
            query.push_str("sort ");
            for (index, sort) in self.sort.iter().enumerate() {
                if index != 0 {
                    query.push(',');
                }
                query.push_str(sort.field);
                query.push(' ');
                if sort.ascending {
                    query.push_str("asc");
                } else {
                    query.push_str("desc");
                }
            }
            query.push(';');
        }

        if let Some(search) = &self.search {
            query.push_str("search ");
            if let Some(column) = search.column {
                query.push_str(column);
                query.push(' ');
            }
            query.push('"');
            query.push_str(&search.query);
            query.push('"');
            query.push(';');
        }

        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_builder_new_is_empty() {
        let builder = QueryBuilder::new();
        assert_eq!(builder.build(), "");
    }

    #[test]
    fn query_builder_all_fields() {
        let builder = QueryBuilder::new().all_fields();
        assert_eq!(builder.build(), "fields *;");
    }

    #[test]
    fn query_builder_specific_fields() {
        let builder = QueryBuilder::new().fields(["id", "name", "rating"]);
        let query = builder.build();
        assert!(query.starts_with("fields "));
        assert!(query.contains("id"));
        assert!(query.contains("name"));
        assert!(query.contains("rating"));
        assert!(query.ends_with(";"));
    }

    #[test]
    fn query_builder_exclude_fields() {
        let builder = QueryBuilder::new().exclude(["screenshots", "videos"]);
        let query = builder.build();
        assert!(query.starts_with("exclude "));
        assert!(query.contains("screenshots"));
        assert!(query.contains("videos"));
        assert!(query.ends_with(";"));
    }

    #[test]
    fn query_builder_filter_with_where_prefix() {
        let builder = QueryBuilder::new().filter("where id = 55");
        assert_eq!(builder.build(), "where id = 55;");
    }

    #[test]
    fn query_builder_filter_without_where_prefix() {
        let builder = QueryBuilder::new().filter("id = 55");
        assert_eq!(builder.build(), "where id = 55;");
    }

    #[test]
    fn query_builder_filter_complex() {
        let builder =
            QueryBuilder::new().filter("first_release_date < 1234567890 & game_type = (0,4,8,9)");
        assert_eq!(
            builder.build(),
            "where first_release_date < 1234567890 & game_type = (0,4,8,9);"
        );
    }

    #[test]
    fn query_builder_limit() {
        let builder = QueryBuilder::new().limit(Limit::new(100).unwrap());
        assert_eq!(builder.build(), "limit 100;");
    }

    #[test]
    fn query_builder_offset() {
        let builder = QueryBuilder::new().offset(50);
        assert_eq!(builder.build(), "offset 50;");
    }

    #[test]
    fn query_builder_sort_asc() {
        let builder = QueryBuilder::new().sort_asc("id");
        assert_eq!(builder.build(), "sort id asc;");
    }

    #[test]
    fn query_builder_sort_desc() {
        let builder = QueryBuilder::new().sort_desc("rating");
        assert_eq!(builder.build(), "sort rating desc;");
    }

    #[test]
    fn query_builder_multiple_sorts() {
        let builder = QueryBuilder::new().sort_asc("name").sort_desc("rating");
        assert_eq!(builder.build(), "sort name asc,rating desc;");
    }

    #[test]
    fn query_builder_search_without_column() {
        let builder = QueryBuilder::new().search("Zelda");
        assert_eq!(builder.build(), "search \"Zelda\";");
    }

    #[test]
    fn query_builder_search_with_column() {
        let builder = QueryBuilder::new().search_column("name", "Zelda");
        assert_eq!(builder.build(), "search name \"Zelda\";");
    }

    #[test]
    fn query_builder_complex_query() {
        let builder = QueryBuilder::new()
            .fields(["id", "name", "rating"])
            .exclude(["screenshots"])
            .filter("rating > 80")
            .limit(Limit::new(50).unwrap())
            .offset(10)
            .sort_desc("rating");
        let query = builder.build();

        assert!(query.contains("fields "));
        assert!(query.contains("id"));
        assert!(query.contains("name"));
        assert!(query.contains("rating"));
        assert!(query.contains("exclude screenshots;"));
        assert!(query.contains("where rating > 80;"));
        assert!(query.contains("limit 50;"));
        assert!(query.contains("offset 10"));
        assert!(query.contains("sort rating desc;"));
    }

    #[test]
    fn query_builder_all_fields_overrides_specific_fields() {
        let builder = QueryBuilder::new().fields(["id", "name"]).all_fields();
        assert_eq!(builder.build(), "fields *;");
    }

    #[test]
    fn limit_new_zero_is_none() {
        assert!(Limit::new(0).is_none());
    }

    #[test]
    fn limit_new_one_is_some() {
        assert!(Limit::new(1).is_some());
        assert_eq!(Limit::new(1).unwrap().get(), 1);
    }

    #[test]
    fn limit_new_max_is_some() {
        assert!(Limit::new(500).is_some());
        assert_eq!(Limit::new(500).unwrap().get(), 500);
    }

    #[test]
    fn limit_new_above_max_is_none() {
        assert!(Limit::new(501).is_none());
    }

    #[test]
    fn limit_constants() {
        assert_eq!(Limit::MIN.get(), 1);
        assert_eq!(Limit::MAX.get(), 500);
    }
}
