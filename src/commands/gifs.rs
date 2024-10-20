mod play;
mod sleep;

use futures::Stream;
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::all::MessageFlags;
use serenity::{CreateMessage, Mentionable, User};
use std::borrow::Cow;
use std::sync::Arc;
use tracing::{error, info, instrument};
use url::Url;

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::Context;
use crate::{cache, BotContextExt};

const GIF_COUNT: u8 = 25;
const MAX_AUTOCOMPLETE_RESULTS: usize = 25;
const BASE_GIF_CONFIG: Config = Config::new()
    .content_filter(ContentFilter::Medium)
    .media_filter(&[MediaFilter::Gif])
    .limit(GIF_COUNT);

#[derive(Debug, thiserror::Error)]
pub(crate) enum GifError {
    #[error(transparent)]
    Tenor(#[from] TenorError),
    #[error("The query \"{0}\" was not allowed")]
    RestrictedQuery(String),
    #[error("no gifs found")]
    NoGifs,
}

// Allow this unused async because autocomplete functions need to be async
#[allow(clippy::unused_async)]
async fn play_autocomplete<'a>(
    _: Context<'_, '_>,
    partial: &'a str,
) -> impl Stream<Item = &'static str> + 'a {
    play::autocomplete(partial)
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
    let (tenor, gif_cache) = ctx.gif_context();
    let output = play::get_command_output(tenor, gif_cache, &mention, game).await?;
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
    let (tenor, gif_cache) = ctx.gif_context();
    let gif = get_gif(tenor, gif_cache, "hurry up", true).await?;
    ctx.reply(format!("{mention}! Hurry up!")).await?;
    send_gif_message(ctx, gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// It's Morbin time
pub(crate) async fn morbin(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let (tenor, gif_cache) = ctx.gif_context();
    let gif = get_gif(tenor, gif_cache, "morbin_time", false).await?;
    ctx.reply(gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Posts a random good night GIF
pub(crate) async fn sleep(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let gif = sleep::get_gif(&ctx.data().gif_cache).await?;
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
pub(crate) async fn update_gif_cache(tenor: &tenor::Client<'_>, gif_cache: &cache::Memory<[Url]>) {
    if let Err(error) = cache_gifs(tenor, gif_cache, Cow::Borrowed("hurry up"), true).await {
        error!("Error caching gifs for hurry up: {error}");
    }
    if let Err(error) = cache_gifs(tenor, gif_cache, Cow::Borrowed("morbin_time"), false).await {
        error!("Error caching gifs for morbin_time: {error}");
    }
    play::update_gif_cache(tenor, gif_cache).await;
    sleep::update_gif_cache(tenor, gif_cache).await;
}

fn mention_or_here(user: Option<&User>) -> Cow<'static, str> {
    user.map_or(Cow::Borrowed("@here"), |u| {
        Cow::Owned(u.mention().to_string())
    })
}

async fn get_gif(
    tenor: &tenor::Client<'_>,
    gif_cache: &cache::Memory<[Url]>,
    query: impl Into<Cow<'static, str>>,
    random: bool,
) -> Result<String, GifError> {
    let gifs = get_gifs(tenor, gif_cache, query, random).await?;
    let url = gifs.choose(&mut thread_rng()).ok_or(GifError::NoGifs)?;
    Ok(url.as_str().into())
}

async fn get_gifs(
    tenor: &tenor::Client<'_>,
    gif_cache: &cache::Memory<[Url]>,
    query: impl Into<Cow<'static, str>>,
    random: bool,
) -> Result<Arc<[Url]>, GifError> {
    let query = query.into();
    if let Some(gifs) = gif_cache.get(&query).await {
        info!("Found \"{query}\" gifs in cache ");
        return Ok(gifs);
    }
    cache_gifs(tenor, gif_cache, query, random).await
}

fn map_gif_to_url(mut gif: Gif) -> Url {
    gif.media_formats
        .remove(&MediaFilter::Gif)
        .map_or(gif.url, |s| s.url)
}

async fn cache_gifs(
    tenor: &tenor::Client<'_>,
    gif_cache: &cache::Memory<[Url]>,
    query: Cow<'static, str>,
    random: bool,
) -> Result<Arc<[Url]>, GifError> {
    let config = BASE_GIF_CONFIG.random(random);
    let gifs = tenor.search(&query, Some(config)).await?;
    let urls: Arc<[Url]> = gifs.into_iter().map(map_gif_to_url).collect();
    let gif_count = urls.len();
    info!(gif_count, "Putting \"{query}\" gifs into cache");
    gif_cache.insert(query, urls.clone()).await;
    Ok(urls)
}
