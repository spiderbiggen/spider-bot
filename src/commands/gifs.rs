use std::sync::Arc;

use chrono::{Datelike, Month, NaiveDate, Utc};
use rand::distributions::{WeightedError, WeightedIndex};
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::all::{
    CommandInteraction, CommandOptionType, CommandType, CreateAutocompleteResponse, CreateCommand,
    CreateCommandOption, CreateInteractionResponse, ResolvedValue,
};
use serenity::client::Context;
use serenity::prelude::Mentionable;
use tracing::{error, info, instrument};

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::messaging::send_reply;
use crate::SpiderBot;

const MAX_AUTOCOMPLETE_RESULTS: usize = 25;
static GAME_AUTOCOMPLETION: &[(&str, &[&str])] = &[
    ("Apex Legends", &["apex legends"]),
    ("Call of Duty", &["cod", "call of duty"]),
    ("Chivalry 2", &["chivalry 2"]),
    ("Halo Infinite", &["halo"]),
    ("League of Legends", &["lol", "league of legends"]),
    ("Overwatch 2", &["overwatch", "ow"]),
    ("Phasmophobia", &["phasmophobia"]),
    ("Rimworld", &["rimworld"]),
    (
        "Sid Meier's Civilization IV",
        &["civilization", "sid meier's civilization iv"],
    ),
    ("Warzone", &["warzone"]),
];

#[instrument(skip_all)]
pub(crate) async fn play_autocomplete(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CommandError> {
    let mut filter: String = String::new();
    if let Some(option) = interaction.data.autocomplete() {
        if option.name == "game" && matches!(option.kind, CommandOptionType::String) {
            filter = option.value.to_lowercase();
        }
    }
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
    interaction.defer(ctx).await?;
    let mut mention: String = String::from("@here");
    let mut game_query: Option<&str> = None;
    for option in &interaction.data.options() {
        match (option.name, &option.value) {
            ("user", ResolvedValue::User(user, _)) => {
                mention = user.mention().to_string();
            }
            ("user", ResolvedValue::Role(role)) => {
                mention = role.mention().to_string();
            }
            ("game", ResolvedValue::String(game)) => {
                game_query.replace(game);
            }
            _ => (),
        }
    }

    let gif = get_gif(bot, &game_query.unwrap_or("games").replace(' ', "_"), false).await?;
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
    interaction.defer(ctx).await?;

    let mut mention: String = String::from("@here");
    for option in &interaction.data.options() {
        if option.name == "user" {
            if let ResolvedValue::User(user, _) = option.value {
                mention = user.mention().to_string();
            }
        }
    }

    let gif = get_gif(bot, "hurry up", true).await?;
    send_reply(ctx, interaction, [format!("{mention}! Hurry up!"), gif]).await
}

#[instrument(skip_all)]
pub(crate) async fn sleep(
    ctx: &Context,
    interaction: &CommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    interaction.defer(ctx).await?;

    let today = Utc::now().date_naive();
    let collection = SLEEP_GIF_COLLECTION.current(today);
    let gif = collection.find(bot).await?;
    send_reply(ctx, interaction, [gif]).await
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

async fn get_gifs(bot: &SpiderBot, query: &str, random: bool) -> Result<Arc<[Gif]>, GifError> {
    if let Some(gifs) = bot.gif_cache.get(query).await {
        info!("Found \"{query}\" gifs in cache ");
        return Ok(gifs);
    }
    let config = Config::default()
        .content_filter(ContentFilter::Medium)
        .media_filter(vec![MediaFilter::Gif])
        .random(random);
    let gifs: Arc<[Gif]> = bot.tenor.search(query, Some(&config)).await?.into();
    bot.gif_cache.insert(query.into(), gifs.clone()).await;
    info!("Put \"{query}\" gifs into cache ");
    Ok(gifs)
}

async fn get_gif(bot: &SpiderBot, query: &str, random: bool) -> Result<String, GifError> {
    let gifs = get_gifs(bot, query, random).await?;
    let single = gifs.choose(&mut thread_rng()).ok_or(GifError::NoGifs)?;
    let url = single
        .media_formats
        .get(&MediaFilter::Gif)
        .map_or(single.url.as_str(), |s| s.url.as_str());
    Ok(url.into())
}

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[Season {
        range: DateRange {
            start: DayOfMonth(15, Month::October),
            end: DayOfMonth(31, Month::October),
        },
        data: CollectionData(&[
            WeightedQuery::single("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif"),
            WeightedQuery(47, GifQuery::Random("halloweensleep")),
            WeightedQuery(47, GifQuery::Random("spookysleep")),
            WeightedQuery(47, GifQuery::Random("horrorsleep")),
        ]),
    }],
    data: CollectionData(&[
        WeightedQuery::single("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif"),
        WeightedQuery(20, GifQuery::Random("sleep")),
        WeightedQuery(20, GifQuery::Random("dogsleep")),
        WeightedQuery(20, GifQuery::Random("catsleep")),
        WeightedQuery(20, GifQuery::Random("rabbitsleep")),
        WeightedQuery(20, GifQuery::Random("ratsleep")),
        WeightedQuery(20, GifQuery::Random("ducksleep")),
        WeightedQuery(20, GifQuery::Random("animalsleep")),
    ]),
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum GifError {
    #[error(transparent)]
    Tenor(#[from] TenorError),
    #[error(transparent)]
    Distribution(#[from] WeightedError),
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

impl PartialEq<NaiveDate> for DateRange {
    fn eq(&self, other: &NaiveDate) -> bool {
        let day = other.day();
        let month = other.month();
        let start_month = self.start.1.number_from_month();
        let end_month = self.end.1.number_from_month();
        (month >= start_month && month <= end_month)
            && !(month == start_month && day < u32::from(self.start.0))
            && !(month == end_month && day > u32::from(self.end.0))
    }
}

#[derive(Debug, Copy, Clone)]
enum GifQuery {
    Single(&'static str),
    Random(&'static str),
}

impl GifQuery {
    async fn find(&self, bot: &SpiderBot) -> Result<String, GifError> {
        match self {
            GifQuery::Single(url) => Ok((*url).to_string()),
            GifQuery::Random(query) => {
                Ok(get_gif(bot, query, matches!(self, GifQuery::Random(_))).await?)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WeightedQuery(u8, GifQuery);

impl WeightedQuery {
    const fn single(url: &'static str) -> Self {
        WeightedQuery(1, GifQuery::Single(url))
    }
}

#[derive(Debug, Clone, Copy)]
struct CollectionData(&'static [WeightedQuery]);

impl CollectionData {
    async fn find(&self, bot: &SpiderBot) -> Result<String, GifError> {
        let dist = WeightedIndex::new(self.0.iter().map(|q| u32::from(q.0)))?;
        let query = self.0[dist.sample(&mut thread_rng())].1;
        query.find(bot).await
    }
}

#[derive(Debug, Clone, Copy)]
struct Season {
    range: DateRange,
    data: CollectionData,
}

#[derive(Debug, Clone, Copy)]
struct GifCollection {
    seasons: &'static [Season],
    data: CollectionData,
}

impl GifCollection {
    fn current(&self, date: NaiveDate) -> &CollectionData {
        self.seasons
            .iter()
            .find(|s| s.range == date)
            .map_or(&self.data, |s| &s.data)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn froggers_chance() {
        let dist =
            WeightedIndex::new(SLEEP_GIF_COLLECTION.data.0.iter().map(|q| u32::from(q.0))).unwrap();
        let mut sum = 0u32;
        let iterations = 100_000u32;
        (0..iterations).for_each(|_| {
            let mut counter = 1;
            loop {
                let query = SLEEP_GIF_COLLECTION.data.0[dist.sample(&mut thread_rng())].1;
                if matches!(
                    query,
                    GifQuery::Single("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif")
                ) {
                    break;
                }
                counter += 1;
            }
            sum += counter;
        });
        #[allow(clippy::cast_possible_truncation)]
        let average_first_appearance = (f64::from(sum) / f64::from(iterations)).round() as i64;
        assert_eq!(average_first_appearance, 141);
    }
}
