mod play;
mod sleep;

use crate::cache::{GifCacheReader, GifCacheWriter};
use crate::commands::CommandError;
use crate::consts::LONG_CACHE_LIFETIME;
use crate::context::{Context, GifCacheExt};
use klipy::models::Format;
use klipy::{Config, Klipy};
use poise::serenity_prelude as serenity;
use serenity::all::MessageFlags;
use serenity::{CreateMessage, Mentionable, User};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;
use url::Url;

const MAX_AUTOCOMPLETE_RESULTS: usize = 25;

const HURRY_QUERY: &str = "hurry up";
const MORBIN_QUERY: &str = "morbin_time";

#[derive(Debug, thiserror::Error)]
pub(crate) enum GifError {
    #[error(transparent)]
    Klipy(#[from] klipy::error::Error),
    #[error("The query \"{0}\" was not allowed")]
    RestrictedQuery(String),
    #[error("no gifs found")]
    NoGifs,
}

async fn play_autocomplete(ctx: Context<'_, '_>, partial: &str) -> Vec<Cow<'static, str>> {
    play::autocomplete(ctx, partial).await
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
pub(crate) async fn refresh_global_gif_cache(klipy: &Klipy<'_>, writer: &GifCacheWriter) {
    sleep::refresh_sleep_gifs(klipy, writer).await;
    play::refresh_play_gifs(klipy, writer).await;
    refresh_gif_cache(klipy, writer).await;
}

async fn refresh_gif_cache(klipy: &Klipy<'_>, writer: &GifCacheWriter) {
    refresh_gif_cache_for_query(klipy, writer, HURRY_QUERY, None).await;
    refresh_gif_cache_for_query(klipy, writer, MORBIN_QUERY, None).await;
}

pub(super) async fn refresh_gif_cache_for_query(
    klipy: &Klipy<'_>,
    writer: &GifCacheWriter,
    query: &str,
    cfg: Option<Config<'_>>,
) -> bool {
    match klipy.search(query, cfg).await {
        Ok(gifs) => {
            let urls: Box<[Arc<Url>]> = gifs
                .into_iter()
                .filter_map(|gif| gif.into_media(Format::Gif))
                .map(Arc::new)
                .collect();
            cache_gifs(writer, query, urls, LONG_CACHE_LIFETIME)
        }
        Err(error) => {
            tracing::error!(query, "Error fetching gifs for: {error}");
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
pub(super) fn get_cached_gif(cache: &GifCacheReader, query: &str) -> Result<Arc<Url>, GifError> {
    cache.get_random(query).ok_or(GifError::NoGifs)
}

#[tracing::instrument(skip(writer, gifs), fields(gifs.len = gifs.len()))]
fn cache_gifs(writer: &GifCacheWriter, key: &str, gifs: Box<[Arc<Url>]>, duration: Duration) -> bool {
    let updated = writer.insert_with_duration(key, gifs, duration);
    if updated {
        tracing::info!("Updated cache");
    }
    updated
}
