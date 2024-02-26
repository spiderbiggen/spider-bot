use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::{Datelike, Month, NaiveDate, Utc};
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::all::{
    CommandInteraction, CommandOptionType, CommandType, CreateAutocompleteResponse, CreateCommand,
    CreateCommandOption, CreateInteractionResponse, ResolvedValue,
};
use serenity::client::Context;
use serenity::prelude::Mentionable;
use tracing::{debug, error, info, instrument, trace};
use url::Url;

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::messaging::send_reply;
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

#[instrument(skip_all)]
pub(crate) async fn play_autocomplete(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CommandError> {
    trace!("filtering autocomplete");
    let mut filter: Option<String> = None;
    if let Some(option) = interaction.data.autocomplete() {
        if option.name == "game" && matches!(option.kind, CommandOptionType::String) {
            filter.replace(option.value.to_lowercase());
        }
    }
    let Some(filter) = filter else {
        debug!("found nothing to autocomplete");
        let interaction_response =
            CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new());
        interaction
            .create_response(ctx, interaction_response)
            .await?;
        return Ok(());
    };

    trace!(filter, "filtering games");
    let autocomplete_response = GAME_AUTOCOMPLETION
        .iter()
        .filter(|(_, cases)| cases.iter().any(|s| s.starts_with(&filter)))
        .take(MAX_AUTOCOMPLETE_RESULTS)
        .fold(CreateAutocompleteResponse::new(), |acc, &(s, _)| {
            acc.add_string_choice(s, s)
        });
    let response = CreateInteractionResponse::Autocomplete(autocomplete_response);
    interaction.create_response(ctx, response).await?;
    Ok(())
}

#[instrument(skip_all)]
pub(crate) async fn play(
    ctx: &Context,
    interaction: &CommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let mut mention = Cow::Borrowed("@here");
    let mut game_query: Option<&str> = None;
    for option in interaction.data.options() {
        match (option.name, option.value) {
            ("user", ResolvedValue::User(user, _)) => {
                mention = Cow::Owned(user.mention().to_string());
            }
            ("user", ResolvedValue::Role(role)) => {
                mention = Cow::Owned(role.mention().to_string());
            }
            ("game", ResolvedValue::String(game)) => {
                game_query.replace(game);
            }
            _ => (),
        }
    }

    let query = game_query.map_or(Cow::Borrowed("games"), |s| Cow::Owned(s.replace(' ', "_")));
    let gif = get_gif(bot, query, false).await?;
    let message = if let Some(game) = game_query {
        format!("{mention}! Let's play some {game}!")
    } else {
        format!("{mention}! Let's play a game!")
    };
    send_reply(ctx, interaction, [message, gif]).await
}

#[instrument(skip_all)]
pub(crate) async fn hurry(
    ctx: &Context,
    interaction: &CommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let mut mention = Cow::Borrowed("@here");
    for option in interaction.data.options() {
        match (option.name, option.value) {
            ("user", ResolvedValue::User(user, _)) => {
                mention = Cow::Owned(user.mention().to_string());
            }
            ("user", ResolvedValue::Role(role)) => {
                mention = Cow::Owned(role.mention().to_string());
            }
            _ => (),
        }
    }

    let gif = get_gif(bot, Cow::Borrowed("hurry up"), true).await?;
    send_reply(ctx, interaction, [format!("{mention}! Hurry up!"), gif]).await
}

#[instrument(skip_all)]
pub(crate) async fn sleep(
    ctx: &Context,
    interaction: &CommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    trace!("looking for sleep gif in cache");
    let gif = SLEEP_GIF_COLLECTION.get_gif(bot).await?;
    debug!("found sleep gif in cache");
    send_reply(ctx, interaction, [gif.into()]).await
}

pub(crate) fn register_commands(commands: &mut Vec<CreateCommand>) {
    commands.push(
        CreateCommand::new("sleep")
            .description("Posts a random good night gif")
            .kind(CommandType::ChatInput),
    );
    commands.push(
        CreateCommand::new("play")
            .description("Tag someone to come play some games")
            .kind(CommandType::ChatInput)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "game",
                    "The game you want to play",
                )
                .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(
                CommandOptionType::Mentionable,
                "user",
                "The user you want to mention",
            )),
    );
    commands.push(
        CreateCommand::new("hurry")
            .description("Hurry up")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(
                CommandOptionType::Mentionable,
                "user",
                "The user you want to mention",
            )),
    );
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
