mod play;
mod sleep;

use crate::Tenor;
use crate::cache::GifCache;
use crate::commands::CommandError;
use crate::consts::LONG_CACHE_LIFETIME;
use crate::context::{Context, GifCacheExt, GifContextExt};
use poise::serenity_prelude as serenity;
use serenity::all::MessageFlags;
use serenity::{CreateMessage, Mentionable, User};
use std::borrow::{Borrow, Cow};
use std::sync::Arc;
use std::time::Duration;
use tenor::models::{Gif, MediaFilter};
use tracing::instrument;
use url::Url;

const MAX_AUTOCOMPLETE_RESULTS: usize = 25;
const RANDOM_CONFIG: tenor::Config = tenor::Config::new().random(true);

const HURRY_QUERY: &str = "hurry up";
const MORBIN_QUERY: &str = "morbin_time";

#[derive(Debug, thiserror::Error)]
pub(crate) enum GifError {
    #[error(transparent)]
    Tenor(#[from] tenor::error::Error),
    #[error("The query \"{0}\" was not allowed")]
    RestrictedQuery(String),
    #[error("no gifs found")]
    NoGifs,
}

fn play_autocomplete(_: Context<'_, '_>, partial: &str) -> impl Future<Output = Vec<&'static str>> {
    futures::future::ready(play::autocomplete(partial))
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Tag someone to play some games with
pub(crate) async fn play(
    ctx: Context<'_, '_>,
    #[description = "Who to play games with"] user: Option<User>,
    #[description = "What game you want to play"]
    #[autocomplete = "play_autocomplete"]
    game: Option<String>,
) -> Result<(), CommandError> {
    let mention = mention_or_here(user.as_ref());
    let output = play::get_command_output(&ctx, mention.as_ref(), game).await?;
    ctx.reply(output.message).await?;
    send_gif_message(ctx, output.gif.to_string()).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
///Tell someone to hurry up
pub(crate) async fn hurry(
    ctx: Context<'_, '_>,
    #[description = "Who should hurry up"] user: Option<User>,
) -> Result<(), CommandError> {
    let gif = get_cached_gif(ctx.gif_cache(), HURRY_QUERY)?;
    let mention = mention_or_here(user.as_ref());
    ctx.reply(format!("{mention}! Hurry up!")).await?;
    send_gif_message(ctx, gif.to_string()).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// It's Morbin time
pub(crate) async fn morbin(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let gif = get_cached_gif(ctx.gif_cache(), MORBIN_QUERY)?;
    ctx.reply(gif.as_str()).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Posts a random good night GIF
pub(crate) async fn sleep(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let gif = sleep::get_gif(ctx.gif_cache()).await?;
    ctx.reply(gif.as_str()).await?;
    Ok(())
}

async fn send_gif_message(
    ctx: Context<'_, '_>,
    gif: impl Into<String>,
) -> Result<(), serenity::Error> {
    let gif_message = CreateMessage::new()
        .flags(MessageFlags::SUPPRESS_NOTIFICATIONS)
        .content(gif);
    ctx.channel_id().send_message(ctx, gif_message).await?;
    Ok(())
}

#[instrument(skip_all)]
pub(crate) async fn update_global_gif_cache(context: &impl GifContextExt<'_>) {
    let (tenor, gif_cache) = context.gif_context();
    refresh_gif_cache(tenor, gif_cache).await;
    play::refresh_gif_cache(tenor, gif_cache).await;
    sleep::refresh_gif_cache(tenor, gif_cache).await;
}

async fn refresh_gif_cache(tenor: &Tenor<'_>, gif_cache: &GifCache) {
    refresh_gif_cache_for_query(tenor, gif_cache, HURRY_QUERY, Some(RANDOM_CONFIG)).await;
    refresh_gif_cache_for_query(tenor, gif_cache, MORBIN_QUERY, None).await;
}

async fn refresh_gif_cache_for_query(
    tenor: &Tenor<'_>,
    gif_cache: &GifCache,
    query: &str,
    cfg: Option<tenor::Config<'_>>,
) -> bool {
    match tenor.search(query, cfg).await {
        Ok(gifs) => {
            cache_gifs(gif_cache, query, gifs, LONG_CACHE_LIFETIME);
            true
        }
        Err(error) => {
            tracing::error!("Error fetching gifs for {query}: {error}");
            false
        }
    }
}

fn mention_or_here(user: Option<&User>) -> Cow<'static, str> {
    user.map_or(Cow::Borrowed("@here"), |u| {
        Cow::Owned(u.mention().to_string())
    })
}

#[inline]
fn get_cached_gif(cache: &GifCache, query: &str) -> Result<Arc<Url>, GifError> {
    cache.get_random(query).ok_or(GifError::NoGifs)
}

fn map_gif_to_url(mut gif: Gif) -> Url {
    gif.media_formats
        .remove(&MediaFilter::Gif)
        .map_or(gif.url, |s| s.url)
}

fn cache_gifs(
    cache: &GifCache,
    key: impl Borrow<str>,
    gifs: impl IntoIterator<Item = Gif>,
    duration: Duration,
) -> bool {
    let key = key.borrow();
    let urls: Box<[Arc<Url>]> = gifs.into_iter().map(map_gif_to_url).map(Arc::new).collect();
    let gif_count = urls.len();

    let updated = cache.insert_with_duration(key, urls, duration);
    if updated {
        tracing::info!(gif_count, r#"Putting "{key}" gifs into cache"#);
    }
    updated
}
