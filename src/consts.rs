use std::time::Duration;

pub(crate) const CACHE_TRIM_INTERVAL: Duration = Duration::from_secs(3600);
pub(crate) const CACHE_LIFETIME: Duration = Duration::from_secs(24 * 3600);
