use std::time::Duration;

pub(crate) const SHORT_CACHE_LIFETIME: Duration = Duration::from_hours(1);
pub(crate) const LONG_CACHE_LIFETIME: Duration = Duration::from_hours(24);
pub(crate) const GIF_COUNT: u8 = 25;
