use std::time::Duration;

pub(crate) const SHORT_CACHE_LIFETIME: Duration = Duration::from_secs(3600);
pub(crate) const LONG_CACHE_LIFETIME: Duration = Duration::from_secs(24 * 3600);
pub(crate) const GIF_COUNT: u8 = 25;
