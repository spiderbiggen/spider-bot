use std::time::Duration;

pub(crate) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(295);
pub(crate) const WATCHER_UPDATE_INTERVAL: Duration = Duration::from_secs(900);
pub(crate) const CACHE_TRIM_INTERVAL: Duration = Duration::from_secs(2995);

pub(crate) const CACHE_LIFETIME: Duration = Duration::from_secs(6000);

pub(crate) const MAX_WATCHER_UPDATE_TASKS: usize = 3;

pub(crate) const MAX_EMBED_CHARS: usize = 2048;
