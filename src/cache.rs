use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::consts;

#[derive(Debug, Clone)]
pub struct CacheKey<T: ?Sized>(Instant, Arc<T>);

#[derive(Debug)]
pub struct MemoryCache<T: ?Sized> {
    map: Arc<RwLock<HashMap<String, CacheKey<T>>>>,
}

impl<T: ?Sized> Clone for MemoryCache<T> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<T: ?Sized> Default for MemoryCache<T> {
    fn default() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<T: ?Sized> MemoryCache<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<Arc<T>> {
        let map = self.map.read().unwrap();
        map.get(key)
            .filter(|&&CacheKey(instant, _)| instant >= Instant::now())
            .map(|CacheKey(_, value)| value.clone())
    }

    pub fn insert(&self, key: String, value: impl Into<Arc<T>>) {
        self.insert_with_duration(key, value, consts::CACHE_LIFETIME)
    }

    pub fn insert_with_duration(&self, key: String, value: impl Into<Arc<T>>, duration: Duration) {
        let expiration = Instant::now() + duration;
        self.insert_with_expiration(key, value, expiration)
    }

    pub fn insert_with_expiration(
        &self,
        key: String,
        value: impl Into<Arc<T>>,
        expiration: Instant,
    ) {
        let mut map = self.map.write().unwrap();
        map.insert(key, CacheKey(expiration, value.into()));
    }

    pub fn trim(&self) {
        let now = Instant::now();
        let mut map = self.map.write().unwrap();
        map.retain(|_, &mut CacheKey(expiration, _)| expiration <= now);
    }
}
