mod play;
mod sleep;

use crate::commands::CommandError;
use crate::consts::{LONG_CACHE_LIFETIME, SHORT_CACHE_LIFETIME};
use crate::context::{Context, GifCacheExt, GifContextExt};
use poise::serenity_prelude as serenity;
use rand::prelude::*;
use serenity::all::MessageFlags;
use serenity::{CreateMessage, Mentionable, User};
use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tenor::models::{Gif, MediaFilter};
use tracing::{debug, error, info, instrument};
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

trait GifSliceExt {
    fn take(&self) -> Result<String, GifError>;
}

impl<T> GifSliceExt for T
where
    T: AsRef<[Url]>,
{
    fn take(&self) -> Result<String, GifError> {
        let url = self
            .as_ref()
            .choose(&mut rand::rng())
            .ok_or(GifError::NoGifs)?;
        Ok(url.to_string())
    }
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
    let output = play::get_command_output(&ctx, &mention, game).await?;
    ctx.reply(output.message).await?;
    send_gif_message(ctx, output.gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
///Tell someone to hurry up
pub(crate) async fn hurry(
    ctx: Context<'_, '_>,
    #[description = "Who should hurry up"] user: Option<User>,
) -> Result<(), CommandError> {
    let mention = mention_or_here(user.as_ref());
    let gif = get_cached_gif(&ctx, HURRY_QUERY).await?;
    ctx.reply(format!("{mention}! Hurry up!")).await?;
    send_gif_message(ctx, gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// It's Morbin time
pub(crate) async fn morbin(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let gif = get_cached_gif(&ctx, MORBIN_QUERY).await?;
    ctx.reply(gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Posts a random good night GIF
pub(crate) async fn sleep(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let gif = sleep::get_gif(&ctx).await?;
    ctx.reply(gif).await?;
    Ok(())
}

async fn send_gif_message(ctx: Context<'_, '_>, gif: String) -> Result<(), serenity::Error> {
    let gif_message = CreateMessage::new()
        .flags(MessageFlags::SUPPRESS_NOTIFICATIONS)
        .content(gif);
    ctx.channel_id().send_message(ctx, gif_message).await?;
    Ok(())
}

#[instrument(skip_all)]
pub(crate) async fn update_gif_cache(context: &impl GifContextExt<'_>) {
    let tenor = context.tenor();
    match tenor.search(HURRY_QUERY, Some(RANDOM_CONFIG)).await {
        Ok(gifs) => {
            cache_gifs(context, HURRY_QUERY, gifs, LONG_CACHE_LIFETIME).await;
        }
        Err(error) => error!("Error caching gifs for {HURRY_QUERY}: {error}"),
    }
    match tenor.search(MORBIN_QUERY, None).await {
        Ok(gifs) => {
            cache_gifs(context, MORBIN_QUERY, gifs, LONG_CACHE_LIFETIME).await;
        }
        Err(error) => error!("Error caching gifs for {MORBIN_QUERY}: {error}"),
    }
    play::update_gif_cache(context).await;
    sleep::update_gif_cache(context).await;
}

fn mention_or_here(user: Option<&User>) -> Cow<'static, str> {
    user.map_or(Cow::Borrowed("@here"), |u| {
        Cow::Owned(u.mention().to_string())
    })
}

async fn get_cached_gif(context: &impl GifContextExt<'_>, query: &str) -> Result<String, GifError> {
    match context.gif_cache().get(query).await {
        Some(cached) => {
            debug!(r#"Found "{query}" gifs in cache"#);
            cached.deref().take()
        }
        None => Err(GifError::NoGifs),
    }
}

async fn update_cached_gifs(
    context: &impl GifContextExt<'_>,
    query: impl Into<Cow<'static, str>>,
    config: Option<tenor::Config<'_>>,
) -> Result<Arc<[Url]>, GifError> {
    let query = query.into();
    let gifs = context.tenor().search(&query, config).await?;
    Ok(cache_gifs(context, query, gifs, SHORT_CACHE_LIFETIME).await)
}

fn map_gif_to_url(mut gif: Gif) -> Url {
    gif.media_formats
        .remove(&MediaFilter::Gif)
        .map_or(gif.url, |s| s.url)
}

async fn cache_gifs(
    context: &impl GifCacheExt,
    key: impl Into<Cow<'static, str>>,
    gifs: impl IntoIterator<Item = Gif>,
    duration: Duration,
) -> Arc<[Url]> {
    let key = key.into();
    let urls: Arc<[Url]> = gifs.into_iter().map(map_gif_to_url).collect();
    info!(gif_count = urls.len(), r#"Putting "{key}" gifs into cache"#);
    context
        .gif_cache()
        .insert_with_duration(key, urls.clone(), duration)
        .await;
    urls
}
