use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::consts;

#[derive(Debug, Clone)]
pub struct Key<T: ?Sized>(Instant, Arc<T>);

#[derive(Debug)]
pub struct Memory<T: ?Sized> {
    map: Arc<RwLock<HashMap<String, Key<T>>>>,
}

impl<T: ?Sized> Clone for Memory<T> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<T: ?Sized> Default for Memory<T> {
    fn default() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<T: ?Sized> Memory<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, key: &str) -> Option<Arc<T>> {
        let map = self.map.read().await;
        map.get(key)
            .filter(|&&Key(instant, _)| instant >= Instant::now())
            .map(|Key(_, value)| value.clone())
    }

    pub async fn insert(&self, key: String, value: impl Into<Arc<T>>) {
        self.insert_with_duration(key, value, consts::CACHE_LIFETIME)
            .await;
    }

    pub async fn insert_with_duration(
        &self,
        key: String,
        value: impl Into<Arc<T>>,
        duration: Duration,
    ) {
        let expiration = Instant::now() + duration;
        self.insert_with_expiration(key, value, expiration).await;
    }

    pub async fn insert_with_expiration(
        &self,
        key: String,
        value: impl Into<Arc<T>>,
        expiration: Instant,
    ) {
        let mut map = self.map.write().await;
        map.insert(key, Key(expiration, value.into()));
    }

    pub async fn trim(&self) {
        let now = Instant::now();
        let mut map = self.map.write().await;
        map.retain(|_, &mut Key(expiration, _)| expiration <= now);
    }
}
