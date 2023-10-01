use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

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

    pub fn get(&self, key: &str) -> Option<Arc<T>> {
        let map = self.map.read().unwrap();
        map.get(key)
            .filter(|&&Key(instant, _)| instant >= Instant::now())
            .map(|Key(_, value)| value.clone())
    }

    pub fn insert(&self, key: String, value: impl Into<Arc<T>>) {
        self.insert_with_duration(key, value, consts::CACHE_LIFETIME);
    }

    pub fn insert_with_duration(&self, key: String, value: impl Into<Arc<T>>, duration: Duration) {
        let expiration = Instant::now() + duration;
        self.insert_with_expiration(key, value, expiration);
    }

    pub fn insert_with_expiration(
        &self,
        key: String,
        value: impl Into<Arc<T>>,
        expiration: Instant,
    ) {
        let mut map = self.map.write().unwrap();
        map.insert(key, Key(expiration, value.into()));
    }

    pub fn trim(&self) {
        let now = Instant::now();
        let mut map = self.map.write().unwrap();
        map.retain(|_, &mut Key(expiration, _)| expiration <= now);
    }
}
