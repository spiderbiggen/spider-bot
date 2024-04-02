use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::{Datelike, Month, NaiveDate, Utc};
use futures::{Stream, StreamExt};
use poise::serenity_prelude as serenity;
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::{CreateMessage, Mentionable, User};
use tracing::{debug, error, info, instrument, trace};
use url::Url;

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::Context;
use crate::{cache, SpiderBot};

const MAX_AUTOCOMPLETE_RESULTS: usize = 25;
static GAME_AUTOCOMPLETION: &[(&str, &[&str])] = &[
    ("Apex Legends", &["apex legends"]),
    ("Call of Duty", &["cod", "call of duty"]),
    ("Chivalry 2", &["chivalry 2"]),
    ("Halo Infinite", &["halo"]),
    ("League of Legends", &["lol", "league of legends"]),
    ("Lethal Company", &["lethal company"]),
    ("Overwatch 2", &["overwatch", "ow"]),
    ("Phasmophobia", &["phasmophobia"]),
    ("Rimworld", &["rimworld"]),
    (
        "Sid Meier's Civilization VI",
        &["civilization", "sid meier's civilization vi"],
    ),
    ("Warzone", &["warzone"]),
];

// Allow this unused async because autocomplete functions need to be async
#[allow(clippy::unused_async)]
async fn play_autocomplete<'a>(
    _: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = &'static str> + 'a {
    futures::stream::iter(GAME_AUTOCOMPLETION)
        .filter(move |(_, cases)| {
            futures::future::ready(cases.iter().any(|s| s.starts_with(partial)))
        })
        .map(|(name, _)| *name)
        .take(MAX_AUTOCOMPLETE_RESULTS)
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Tag someone to play some games with
pub(crate) async fn play(
    ctx: Context<'_>,
    #[description = "Who to play games with"] user: Option<User>,
    #[description = "What game you want to play"]
    #[autocomplete = "play_autocomplete"]
    game: Option<String>,
) -> Result<(), CommandError> {
    let query = game
        .as_ref()
        .map_or(Cow::Borrowed("games"), |s| Cow::Owned(s.replace(' ', "_")));
    let mention = user.map_or(Cow::Borrowed("@here"), |u| {
        Cow::Owned(u.mention().to_string())
    });
    let gif = get_gif(ctx.data(), query, false).await?;
    let message = if let Some(game) = &game {
        format!("{mention}! Let's play some {game}!")
    } else {
        format!("{mention}! Let's play a game!")
    };
    ctx.reply(message).await?;
    ctx.channel_id()
        .send_message(ctx, CreateMessage::new().content(gif))
        .await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
///Tell someone to hurry up
pub(crate) async fn hurry(
    ctx: Context<'_>,
    #[description = "Who should hurry up"] user: Option<User>,
) -> Result<(), CommandError> {
    let mention = user.map_or(Cow::Borrowed("@here"), |u| {
        Cow::Owned(u.mention().to_string())
    });

    let gif = get_gif(ctx.data(), Cow::Borrowed("hurry up"), true).await?;
    ctx.reply(format!("{mention}! Hurry up!")).await?;
    ctx.channel_id()
        .send_message(ctx, CreateMessage::new().content(gif))
        .await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// It's Morbin time
pub(crate) async fn morbin(ctx: Context<'_>) -> Result<(), CommandError> {
    let gif = get_gif(ctx.data(), Cow::Borrowed("morbin_time"), false).await?;
    ctx.reply(gif).await?;
    Ok(())
}

#[instrument(skip_all)]
#[poise::command(slash_command)]
/// Posts a random good night gif
pub(crate) async fn sleep(ctx: Context<'_>) -> Result<(), CommandError> {
    trace!("looking for sleep gif in cache");
    let gif = SLEEP_GIF_COLLECTION.get_gif(ctx.data()).await?;
    debug!("found sleep gif in cache");
    ctx.reply(gif).await?;
    Ok(())
}

async fn get_gifs(
    bot: &SpiderBot,
    query: Cow<'static, str>,
    random: bool,
) -> Result<Arc<[Url]>, GifError> {
    if let Some(gifs) = bot.gif_cache.get(&query).await {
        info!("Found \"{query}\" gifs in cache ");
        return Ok(gifs);
    }
    let config = Config::default()
        .content_filter(ContentFilter::Medium)
        .media_filter(vec![MediaFilter::Gif])
        .random(random);
    let gifs = bot.tenor.search(&query, Some(&config)).await?;
    let urls: Arc<[Url]> = gifs.into_iter().map(map_gif).collect::<Vec<_>>().into();
    info!("Putting \"{query}\" gifs into cache");
    bot.gif_cache.insert(query, urls.clone()).await;
    Ok(urls)
}

fn map_gif(mut gif: Gif) -> Url {
    gif.media_formats
        .remove(&MediaFilter::Gif)
        .map_or(gif.url, |s| s.url)
}

async fn get_gif(
    bot: &SpiderBot,
    query: Cow<'static, str>,
    random: bool,
) -> Result<String, GifError> {
    let gifs = get_gifs(bot, query, random).await?;
    let url = gifs.choose(&mut thread_rng()).ok_or(GifError::NoGifs)?;
    Ok(url.as_str().into())
}

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    name: "sleep gifs",
    ratio_override: Some(RatioQuery {
        query: "https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif",
        numerator: 1,
        denominator: 150,
    }),
    seasons: &[Season {
        name: "halloween",
        range: DateRange {
            start: DayOfMonth(15, Month::October),
            end: DayOfMonth(31, Month::October),
        },
        data: &["halloweensleep", "spookysleep", "horrorsleep"],
    }],
    data: &[
        "sleep",
        "dogsleep",
        "catsleep",
        "rabbitsleep",
        "ratsleep",
        "ducksleep",
        "animalsleep",
    ],
};

#[instrument(skip_all)]
pub(crate) async fn update_sleep_cache(
    tenor: &tenor::Client,
    gif_cache: &cache::Memory<[Url]>,
) -> Result<(), GifError> {
    const GIF_COUNT: u8 = 25;

    debug!("Updating sleep gifs cache");
    let date = Utc::now().date_naive();
    let collection = SLEEP_GIF_COLLECTION.current(date);

    let mut gif_collection: HashSet<Url> =
        HashSet::with_capacity(collection.len() * usize::from(GIF_COUNT));
    let config = Config::default()
        .content_filter(ContentFilter::Medium)
        .media_filter(vec![MediaFilter::Gif])
        .limit(GIF_COUNT)
        .random(true);
    for &query in collection {
        let gifs = tenor.search(query, Some(&config)).await?;
        gif_collection.extend(gifs.into_iter().map(|gif| gif.url));
    }
    let name = SLEEP_GIF_COLLECTION.name;
    let gif_count = gif_collection.len();
    gif_cache
        .insert(name, gif_collection.into_iter().collect::<Vec<_>>())
        .await;
    info!(gif_count, "Updated sleep gifs cache");
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum GifError {
    #[error(transparent)]
    Tenor(#[from] TenorError),
    #[error("no gifs found")]
    NoGifs,
}

#[derive(Debug, Copy, Clone)]
struct DayOfMonth(u8, Month);

#[derive(Debug, Copy, Clone)]
pub(crate) struct DateRange {
    start: DayOfMonth,
    end: DayOfMonth,
}

impl DateRange {
    fn contains(self, other: NaiveDate) -> bool {
        let day = other.day();
        let month = other.month();
        let start_month = self.start.1.number_from_month();
        let end_month = self.end.1.number_from_month();
        (month >= start_month && month <= end_month)
            && !(month == start_month && day < u32::from(self.start.0))
            && !(month == end_month && day > u32::from(self.end.0))
    }
}

#[derive(Debug, Clone, Copy)]
struct GifCollection<'a> {
    name: &'static str,
    ratio_override: Option<RatioQuery>,
    seasons: &'a [Season<'a>],
    data: CollectionData<'a>,
}

#[derive(Debug, Copy, Clone)]
struct RatioQuery {
    query: &'static str,
    numerator: u32,
    denominator: u32,
}

#[derive(Debug, Clone, Copy)]
struct Season<'a> {
    name: &'static str,
    range: DateRange,
    data: CollectionData<'a>,
}

type CollectionData<'a> = &'a [&'a str];

impl<'a> GifCollection<'a> {
    #[must_use]
    #[instrument(skip_all)]
    fn current(&self, date: NaiveDate) -> &[&str] {
        let season = self.seasons.iter().find(|s| s.range.contains(date));
        match season {
            None => self.data,
            Some(season) => {
                debug!("found gifs for {} season", season.name);
                season.data
            }
        }
    }

    #[instrument(skip_all, err)]
    async fn get_gif(&self, bot: &SpiderBot) -> Result<Cow<'static, str>, GifError> {
        if let Some(query) = self.get_override() {
            debug!("Found gif override");
            return Ok(Cow::Borrowed(query));
        }
        let collection = bot.gif_cache.get(self.name).await.ok_or(GifError::NoGifs)?;
        let gif = collection
            .choose(&mut thread_rng())
            .ok_or(GifError::NoGifs)?;
        Ok(gif.as_str().to_string().into())
    }

    #[must_use]
    fn get_override(&self) -> Option<&'static str> {
        self.ratio_override
            .filter(|ratio| thread_rng().gen_ratio(ratio.numerator, ratio.denominator))
            .map(|query| query.query)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn froggers_chance() {
        let mut sum = 0u32;
        let iterations = 100_000u32;
        (0..iterations).for_each(|_| {
            let mut counter = 1;
            loop {
                if SLEEP_GIF_COLLECTION.get_override()
                    == Some("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif")
                {
                    break;
                }
                counter += 1;
            }
            sum += counter;
        });
        let average_rolls = f64::from(sum) / f64::from(iterations);
        eprintln!("Froggers average rolls[iterations={iterations}]: {average_rolls:.2}");
        assert!(average_rolls > 149.0 && average_rolls < 151.0);
    }
}
