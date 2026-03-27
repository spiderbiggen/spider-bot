use crate::consts;
use dashmap::DashMap;
use rand::RngExt;
use rustc_hash::FxBuildHasher;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use url::Url;

#[derive(Debug)]
struct Value {
    fresh_until: Instant,
    data: Box<[Arc<Url>]>,
}

enum WriteOp {
    Insert {
        key: String,
        value: Box<[Arc<Url>]>,
        fresh_until: Instant,
    },
    Trim,
}

/// Read-only handle to the GIF cache. Cheap to clone.
#[derive(Debug, Clone)]
pub struct GifCacheReader {
    inner: Arc<DashMap<String, Value, FxBuildHasher>>,
}

/// Write handle to the GIF cache. All writes are serialised through a single
/// background task. Cheap to clone (wraps an `mpsc::Sender`).
#[derive(Debug, Clone)]
pub struct GifCacheWriter {
    tx: mpsc::Sender<WriteOp>,
}

/// Create a linked reader/writer pair and spawn the single writer task.
/// Must be called inside a Tokio runtime.
pub fn create_gif_cache() -> (GifCacheReader, GifCacheWriter) {
    let inner = Arc::new(DashMap::with_hasher(FxBuildHasher));
    let (tx, rx) = mpsc::channel(128);
    tokio::spawn(cache_writer_task(Arc::clone(&inner), rx));
    (GifCacheReader { inner }, GifCacheWriter { tx })
}

async fn cache_writer_task(
    inner: Arc<DashMap<String, Value, FxBuildHasher>>,
    mut rx: mpsc::Receiver<WriteOp>,
) {
    while let Some(op) = rx.recv().await {
        match op {
            WriteOp::Insert {
                key,
                value,
                fresh_until,
            } => {
                inner.insert(key, Value { fresh_until, data: value });
            }
            WriteOp::Trim => {
                let now = Instant::now();
                inner.retain(|_, v| v.fresh_until >= now);

                // Shrink to fit is a relatively expensive operation.
                // Only shrink if we're significantly over-allocated
                // and have enough elements to justify the cost of reallocation.
                let (cap, len) = (inner.capacity(), inner.len());
                if cap > 64 && len * 4 < cap {
                    inner.shrink_to_fit();
                }
            }
        }
    }
}

impl GifCacheReader {
    pub fn get_random(&self, key: impl Borrow<str>) -> Option<Arc<Url>> {
        self.inner
            .view(key.borrow(), |_, v| {
                let data = &v.data;
                if data.is_empty() {
                    return None;
                }
                let index = rand::rng().random_range(0..data.len());
                Some(Arc::clone(&data[index]))
            })
            .flatten()
    }
}

impl GifCacheWriter {
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

    #[tracing::instrument(skip_all)]
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
        if let Err(e) = self.tx.try_send(WriteOp::Insert { key, value, fresh_until }) {
            tracing::warn!("Failed to queue gif cache insert: {e}");
            return false;
        }
        true
    }

    pub fn trim(&self) {
        if let Err(e) = self.tx.try_send(WriteOp::Trim) {
            tracing::warn!("Failed to queue gif cache trim: {e}");
        }
    }
}
