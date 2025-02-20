use std::time::Duration;
use tenor::Config;
use tenor::models::{ContentFilter, MediaFilter};

pub(crate) const SHORT_CACHE_LIFETIME: Duration = Duration::from_secs(3600);
pub(crate) const LONG_CACHE_LIFETIME: Duration = Duration::from_secs(24 * 3600);
pub(crate) const GIF_COUNT: u8 = 25;
pub(crate) const BASE_GIF_CONFIG: Config = Config::new()
    .content_filter(ContentFilter::Medium)
    .media_filter(&[MediaFilter::Gif])
    .limit(GIF_COUNT);
