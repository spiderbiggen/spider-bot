use crate::consts;
use dashmap::DashMap;
use rand::Rng;
use rustc_hash::FxBuildHasher;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::instrument;
use url::Url;

#[derive(Debug, Clone)]
pub struct Value {
    fresh_until: Instant,
    data: Box<[Arc<Url>]>,
}

#[derive(Debug)]
pub struct GifCache {
    map: Arc<DashMap<String, Value, FxBuildHasher>>,
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
            map: Arc::new(DashMap::with_hasher(FxBuildHasher)),
        }
    }
}

impl GifCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_random(&self, key: impl Borrow<str>) -> Option<Arc<Url>> {
        self.map
            .view(key.borrow(), |_, v| {
                let data = &v.data;
                if data.is_empty() {
                    return None;
                }
                let lengths = data.len();
                let index = rand::rng().random_range(0..lengths);
                Some(Arc::clone(&data[index]))
            })
            .flatten()
    }

    #[allow(dead_code)]
    pub fn insert(&self, key: impl Into<String>, value: Box<[Arc<Url>]>) -> bool {
        self.insert_with_duration(key, value, consts::SHORT_CACHE_LIFETIME)
    }

    pub fn insert_with_duration(
        &self,
        key: impl Into<String>,
        value: Box<[Arc<Url>]>,
        duration: Duration,
    ) -> bool {
        let fresh_until = Instant::now() + duration;
        self.insert_with_freshness(key, value, fresh_until)
    }

    #[instrument(skip_all, fields(key))]
    pub fn insert_with_freshness(
        &self,
        key: impl Into<String>,
        value: Box<[Arc<Url>]>,
        fresh_until: Instant,
    ) -> bool {
        let key = key.into();
        if value.is_empty() {
            tracing::Span::current().record("key", &key);
            tracing::warn!("Tried to insert empty gif collection");
            return false;
        }

        tracing::Span::current().record("key", &key);
        self.map.insert(
            key,
            Value {
                fresh_until,
                data: value,
            },
        );
        true
    }

    pub fn trim(&self) {
        let now = Instant::now();
        self.map.retain(|_, v| v.fresh_until >= now);

        // Shrink to fit is a relatively expensive operation.
        // only shrink if we're significantly over-allocated
        // and have enough elements to justify the cost of reallocation.
        let (cap, len) = (self.map.capacity(), self.map.len());
        if cap > 64 && len * 4 < cap {
            self.map.shrink_to_fit();
        }
    }
}
