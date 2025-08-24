use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::consts;

#[derive(Debug, Clone)]
pub struct Value<T: ?Sized>(Instant, Arc<T>);

#[derive(Debug)]
pub struct Memory<K, T: ?Sized> {
    map: Arc<RwLock<HashMap<K, Value<T>>>>,
}

impl<K, T: ?Sized> Clone for Memory<K, T> {
    fn clone(&self) -> Self {
        Self {
            map: Arc::clone(&self.map),
        }
    }
}

impl<K, T: ?Sized> Default for Memory<K, T> {
    fn default() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K, T> Memory<K, T>
where
    T: ?Sized,
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get<R>(&self, key: &R) -> Option<Arc<T>>
    where
        R: Eq + Hash + ?Sized,
        K: Borrow<R>,
    {
        let map = self.map.read().await;
        map.get(key)
            .filter(|&&Value(instant, _)| instant >= Instant::now())
            .map(|Value(_, value)| Arc::clone(value))
    }

    #[expect(dead_code)]
    pub async fn insert<O: Into<K>, V: Into<Arc<T>>>(&self, key: O, value: V) {
        self.insert_with_duration(key, value, consts::SHORT_CACHE_LIFETIME)
            .await;
    }

    pub async fn insert_with_duration<O, V>(&self, key: O, value: V, duration: Duration)
    where
        O: Into<K>,
        V: Into<Arc<T>>,
    {
        let expiration = Instant::now() + duration;
        self.insert_with_expiration(key, value, expiration).await;
    }

    pub async fn insert_with_expiration<O, V>(&self, key: O, value: V, expiration: Instant)
    where
        O: Into<K>,
        V: Into<Arc<T>>,
    {
        let mut map = self.map.write().await;
        map.insert(key.into(), Value(expiration, value.into()));
    }

    pub async fn trim(&self) {
        let now = Instant::now();
        let mut map = self.map.write().await;
        map.retain(|_, &mut Value(expiration, _)| expiration >= now);
        map.shrink_to_fit();
    }
}
