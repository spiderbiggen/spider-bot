use crate::consts;
use rand::prelude::IteratorRandom;
use rustc_hash::FxHashMap;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use url::Url;

#[derive(Debug, Clone)]
pub struct Value {
    fresh_until: Instant,
    data: Arc<[Url]>,
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

    #[allow(dead_code)]
    pub async fn get(&self, key: impl Borrow<str>) -> Option<Arc<[Url]>> {
        let map = self.map.read().await;
        map.get(key.borrow()).map(|v| Arc::clone(&v.data))
    }

    pub async fn get_random(&self, key: impl Borrow<str>) -> Option<Url> {
        let map = self.map.read().await;
        map.get(key.borrow())
            .and_then(|v| v.data.iter().choose(&mut rand::rng()).cloned())
    }

    #[allow(dead_code)]
    pub async fn insert(&self, key: impl Into<String>, value: impl Into<Arc<[Url]>>) {
        self.insert_with_duration(key, value, consts::SHORT_CACHE_LIFETIME)
            .await;
    }

    pub async fn insert_with_duration(
        &self,
        key: impl Into<String>,
        value: impl Into<Arc<[Url]>>,
        duration: Duration,
    ) {
        let fresh_until = Instant::now() + duration;
        self.insert_with_freshness(key, value, fresh_until).await;
    }

    pub async fn insert_with_freshness(
        &self,
        key: impl Into<String>,
        value: impl Into<Arc<[Url]>>,
        fresh_until: Instant,
    ) {
        let mut map = self.map.write().await;
        map.insert(
            key.into(),
            Value {
                fresh_until,
                data: value.into(),
            },
        );
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
