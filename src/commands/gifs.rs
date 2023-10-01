use std::sync::Arc;

use chrono::{Datelike, Month, NaiveDate, Utc};
use rand::distributions::{WeightedError, WeightedIndex};
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serenity::builder::CreateApplicationCommands;
use serenity::client::Context;
use serenity::model::application::command::CommandType;
use serenity::model::prelude::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::prelude::autocomplete::AutocompleteInteraction;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::Mentionable;
use tracing::{error, info, instrument};

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::messaging::send_reply;
use crate::SpiderBot;

#[instrument(skip_all)]
pub(crate) async fn play_autocomplete(
    ctx: &Context,
    interaction: &AutocompleteInteraction,
) -> Result<(), CommandError> {
    let mut filter: String = String::new();
    for option in &interaction.data.options {
        if option.name == "game" && option.focused {
            if let Some(CommandDataOptionValue::String(input)) = option.resolved.as_ref() {
                filter = input.to_lowercase();
            }
        }
    }
    let mut completions: Vec<_> = GAME_AUTOCOMPLETION
        .iter()
        .filter(|(_, cases)| cases.iter().any(|s| s.starts_with(&filter)))
        .map(|(s, _)| s)
        .collect();
    completions.sort();
    interaction
        .create_autocomplete_response(ctx, |response| {
            for s in completions {
                response.add_string_choice(s, s);
            }
            response
        })
        .await?;
    Ok(())
}

#[instrument(skip_all)]
pub(crate) async fn play(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let mut mention: String = String::from("@here");
    let mut game_query: Option<&str> = None;
    for option in &interaction.data.options {
        let Some(resolved) = &option.resolved else {
            continue;
        };
        match (option.name.as_str(), resolved) {
            ("user", CommandDataOptionValue::User(user, _)) => {
                mention = user.mention().to_string();
            }
            ("user", CommandDataOptionValue::Role(role)) => {
                mention = role.mention().to_string();
            }
            ("game", CommandDataOptionValue::String(game)) => {
                game_query.replace(game);
            }
            _ => (),
        }
    }

    interaction.defer(ctx).await?;
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
    interaction: &ApplicationCommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let mut mention: String = String::from("@here");
    for option in &interaction.data.options {
        if option.name == "user" {
            if let Some(CommandDataOptionValue::User(user, _)) = &option.resolved {
                mention = user.mention().to_string();
            }
        }
    }

    interaction.defer(ctx).await?;
    let gif = get_gif(bot, "hurry up", true).await?;
    send_reply(ctx, interaction, [format!("{mention}! Hurry up!"), gif]).await
}

#[instrument(skip_all)]
pub(crate) async fn sleep(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let today = Utc::now().date_naive();
    let collection = SLEEP_GIF_COLLECTION.current(today);
    interaction.defer(ctx).await?;
    let gif = collection.find(bot).await?;
    send_reply(ctx, interaction, [gif]).await
}

pub(crate) fn register_commands(
    commands: &mut CreateApplicationCommands,
) -> &mut CreateApplicationCommands {
    commands.create_application_command(|command| {
        command
            .name("sleep")
            .description("Posts a random good night gif")
            .kind(CommandType::ChatInput)
    });
    commands.create_application_command(|command| {
        command
            .name("play")
            .description("Tag someone to come play some games")
            .kind(CommandType::ChatInput)
            .create_option(|option| {
                option
                    .name("game")
                    .description("The game you want to play")
                    .set_autocomplete(true)
                    .kind(CommandOptionType::String)
            })
            .create_option(|option| {
                option
                    .name("user")
                    .description("The user you want to mention")
                    .kind(CommandOptionType::Mentionable)
            })
    });
    commands.create_application_command(|command| {
        command
            .name("hurry")
            .description("Hurry up")
            .kind(CommandType::ChatInput)
            .create_option(|option| {
                option
                    .name("user")
                    .description("The user you want to mention")
                    .kind(CommandOptionType::Mentionable)
            })
    });
    commands
}

async fn get_gifs(bot: &SpiderBot, query: &str, random: bool) -> Result<Arc<[Gif]>, GifError> {
    if let Some(gifs) = bot.gif_cache.get(query) {
        info!("Found \"{query}\" gifs in cache ");
        return Ok(gifs);
    }
    let config = Config::default()
        .content_filter(ContentFilter::Medium)
        .media_filter(vec![MediaFilter::Gif])
        .random(random);
    let gifs: Arc<[Gif]> = bot.tenor.search(query, Some(&config)).await?.into();
    bot.gif_cache.insert(query.into(), gifs.clone());
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

static GAME_AUTOCOMPLETION: &[(&str, &[&str])] = &[
    ("League of Legends", &["lol", "league of legends"]),
    ("Chivalry 2", &["chivalry 2"]),
    ("Overwatch 2", &["overwatch", "ow"]),
    (
        "Sid Meier's Civilization IV",
        &["civilization", "sid meier's civilization iv"],
    ),
    ("Phasmophobia", &["phasmophobia"]),
    ("Rimworld", &["rimworld"]),
    ("Halo Infinite", &["halo"]),
    ("Apex Legends", &["apex legends"]),
    ("Warzone", &["warzone"]),
    ("Call of Duty", &["cod", "call of duty"]),
];

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
