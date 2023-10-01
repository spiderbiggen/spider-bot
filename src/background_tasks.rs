use crate::consts::CACHE_TRIM_INTERVAL;
use crate::SpiderBot;

/// Core task spawning function. Creates a set of periodically recurring tasks on their own threads.
///
/// ### Arguments
///
/// - `bot` - the bot instance to delegate to tasks
pub(crate) fn run_periodic_tasks(bot: &SpiderBot) {
    let c = bot.gif_cache.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CACHE_TRIM_INTERVAL);

        loop {
            interval.tick().await;
            c.trim();
        }
    });
}
