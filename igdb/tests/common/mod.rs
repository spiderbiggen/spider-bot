use governor::{DefaultDirectRateLimiter, RateLimiter};
use igdb::Igdb;
use std::env::var;
use std::sync::{Arc, OnceLock};

pub fn setup() -> Igdb<'static> {
    static ONCE_LOCK: OnceLock<(&'static str, &'static str)> = OnceLock::new();
    let (client_id, client_secret) = ONCE_LOCK.get_or_init(|| {
        let _ = dotenv::dotenv();
        let client_id = client_id();
        let client_secret = client_secret();
        (client_id, client_secret)
    });
    Igdb::new_with_governor(client_id, client_secret, governor()).unwrap()
}

fn governor() -> Arc<DefaultDirectRateLimiter> {
    static ONCE_LOCK: OnceLock<Arc<DefaultDirectRateLimiter>> = OnceLock::new();
    Arc::clone(ONCE_LOCK.get_or_init(|| Arc::new(RateLimiter::direct(igdb::IGDB_RATE_LIMIT))))
}

fn client_id() -> &'static str {
    let boxed = var("IGDB_CLIENT_ID")
        .expect("IGDB_CLIENT_ID should be set as an environment variable")
        .into_boxed_str();
    Box::leak(boxed)
}

fn client_secret() -> &'static str {
    let boxed = var("IGDB_CLIENT_SECRET")
        .expect("IGDB_CLIENT_ID should be set as an environment variable")
        .into_boxed_str();
    Box::leak(boxed)
}
