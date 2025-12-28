use crate::consts;
use rand::Rng;
use rustc_hash::FxHashMap;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::instrument;
use url::Url;

#[derive(Debug, Clone)]
pub struct Value {
    fresh_until: Instant,
    data: Box<[Url]>,
}

#[derive(Debug)]
pub struct GifCache {
    map: Arc<RwLock<FxHashMap<String, Value>>>,
}

impl Clone for GifCache {
    fn clone(&self) -> Self {
        Self {
            map: Arc::clone(&self.map),
        }
    }
}

impl Default for GifCache {
    fn default() -> Self {
        Self {
            map: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }
}

impl GifCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_random(&self, key: impl Borrow<str>) -> Option<Url> {
        let map = self.map.read().await;
        let Value { data, .. } = map.get(key.borrow())?;
        if data.is_empty() {
            return None;
        }
        let lengths = data.len();
        let index = rand::rng().random_range(0..lengths);
        Some(data[index].clone())
    }

    #[allow(dead_code)]
    pub async fn insert(&self, key: impl Into<String>, value: impl Into<Box<[Url]>>) -> bool {
        self.insert_with_duration(key, value, consts::SHORT_CACHE_LIFETIME)
            .await
    }

    pub async fn insert_with_duration(
        &self,
        key: impl Into<String>,
        value: impl Into<Box<[Url]>>,
        duration: Duration,
    ) -> bool {
        let fresh_until = Instant::now() + duration;
        self.insert_with_freshness(key, value, fresh_until).await
    }

    #[instrument(skip_all, fields(key))]
    pub async fn insert_with_freshness(
        &self,
        key: impl Into<String>,
        value: impl Into<Box<[Url]>>,
        fresh_until: Instant,
    ) -> bool {
        let key = key.into();
        let data = value.into();
        if data.is_empty() {
            tracing::Span::current().record("key", &key);
            tracing::warn!("Tried to insert empty gif collection");
            return false;
        }

        tracing::Span::current().record("key", &key);
        let mut map = self.map.write().await;
        map.insert(key, Value { fresh_until, data });
        true
    }

    pub async fn trim(&self) {
        let mut map = self.map.write().await;

        let now = Instant::now();
        map.retain(|_, v| v.fresh_until >= now);

        // Shrink to fit is a relatively expensive operation.
        // Capacity management: only shrink if we're significantly over-allocated
        // and have enough elements to justify the cost of reallocation.
        if map.capacity() > 64 && map.len() * 2 < map.capacity() {
            map.shrink_to_fit();
        }
    }
}
