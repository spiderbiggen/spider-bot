use std::borrow::Cow;
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
use serenity::utils::Color;
use tracing::{debug, error, info, instrument};

use tenor::error::Error as TenorError;
use tenor::models::{ContentFilter, Gif, MediaFilter};
use tenor::Config;

use crate::commands::CommandError;
use crate::SpiderBot;

#[instrument(skip_all)]
pub(crate) async fn play_autocomplete(
    ctx: &Context,
    interaction: &AutocompleteInteraction,
) -> Result<(), CommandError> {
    let mut filter: String = String::from("");
    for option in interaction.data.options.iter() {
        if option.name == "game" && option.focused {
            if let Some(CommandDataOptionValue::String(input)) = option.resolved.as_ref() {
                filter = input.to_lowercase();
            }
        }
    }
    info!("autocompleting game: {filter}");
    interaction
        .create_autocomplete_response(ctx, |response| {
            GAME_AUTOCOMPLETION
                .iter()
                .filter(|s| s.to_lowercase().contains(&filter))
                .for_each(|s| {
                    response.add_string_choice(s, s);
                });
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
    let mut username: String = String::from("@here");
    let mut game_query: &str = "games";
    for option in interaction.data.options.iter() {
        if option.name == "user" {
            if let Some(CommandDataOptionValue::User(user, _)) = &option.resolved {
                username = user.mention().to_string()
            }
        } else if option.name == "game" {
            if let Some(CommandDataOptionValue::String(game)) = &option.resolved {
                game_query = game;
            }
        }
    }

    interaction.defer(ctx).await?;
    let gif = get_gif(bot, game_query, true).await?;
    info!(gif =?gif, "found gif to send to {username}");
    interaction
        .edit_original_interaction_response(ctx, |response| {
            response.embed(|embed| {
                embed
                    .image(gif)
                    .author(|author| {
                        author.name(&interaction.user.name).icon_url(
                            interaction
                                .user
                                .avatar_url()
                                .as_ref()
                                .unwrap_or(&interaction.user.default_avatar_url()),
                        )
                    })
                    .description(format_args!("{}! Let's play a game!", username))
                    .colour(Color::DARK_GREEN);
                embed
            })
        })
        .await?;
    Ok(())
}

#[instrument(skip_all)]
pub(crate) async fn sleep(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    bot: &SpiderBot,
) -> Result<(), CommandError> {
    let today = Utc::today().naive_utc();
    let collection = SLEEP_GIF_COLLECTION.current(&today);
    interaction.defer(ctx).await?;
    let (gif, message) = collection.find(bot).await?;
    info!(gif =?gif, "found gif to send");
    interaction
        .edit_original_interaction_response(ctx, |response| {
            response.embed(|embed| {
                embed
                    .image(gif)
                    .author(|author| {
                        author.name(&interaction.user.name).icon_url(
                            interaction
                                .user
                                .avatar_url()
                                .as_ref()
                                .unwrap_or(&interaction.user.default_avatar_url()),
                        )
                    })
                    .colour(Color::FOOYOO);
                if let Some(message) = message {
                    embed.description(message);
                }
                embed
            })
        })
        .await?;
    Ok(())
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
                    .name("user")
                    .description("The user you want to mention")
                    .kind(CommandOptionType::User)
            })
            .create_option(|option| {
                option
                    .name("game")
                    .description("The game you want to play")
                    .set_autocomplete(true)
                    .kind(CommandOptionType::String)
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
        debug!("cached gifs for {query}");
        return Ok(gifs);
    }
    let config = Config::default()
        .content_filter(ContentFilter::Medium)
        .media_filter(vec![MediaFilter::Gif])
        .random(random);
    let gifs = bot.tenor.search(query, Some(&config)).await?;
    let gifs: Arc<[Gif]> = gifs.into();
    bot.gif_cache.insert(query.into(), gifs.clone());
    Ok(gifs)
}

async fn get_gif(bot: &SpiderBot, query: &str, random: bool) -> Result<String, GifError> {
    let gifs = get_gifs(bot, query, random).await?;
    let single = gifs.choose(&mut thread_rng()).ok_or(GifError::NoGifs)?;
    let url = single
        .media_formats
        .get(&MediaFilter::Gif)
        .map(|s| s.url.as_str())
        .unwrap_or(single.url.as_str());
    Ok(url.into())
}

static GAME_AUTOCOMPLETION: &[&str] = &[
    "League of Legends",
    "Overwatch",
    "Phasmophobia",
    "Rimworld",
    "Halo Infinite",
    "Chivalry 2",
    "Apex Legends",
    "Sid Meier's Civilization IV",
    "Warzone",
    "Call of Duty",
];

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[Season {
        range: DateRange {
            start: DayOfMonth(1, Month::October),
            end: DayOfMonth(31, Month::October),
        },
        data: CollectionData {
            queries: &[
                WeightedQuery::single("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif"),
                WeightedQuery(47, GifQuery::Random("halloweensleep")),
                WeightedQuery(47, GifQuery::Random("spookysleep")),
                WeightedQuery(47, GifQuery::Random("horrorsleep")),
            ],
            messages: &[
                "Hope your night is so happy, it makes you glow from the inside out",
                "Wishing you a night that is so fun it's scary.",
                "Who let the ghosts out? They haunt all day and party all night!",
                "The witching hour has begun, go and spread some spooky fun!",
                "It’s that time of the year, of spooky décor, may your night be filled with horror!",
                "A pumpkin a day keeps little ghosts away.",
                "Ghosts are fun, but they can also be little sheets.",
                "Dead & breakfast: Rooms available.",
            ],
        },
    }],
    data: CollectionData {
        queries: &[
            WeightedQuery::single("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif"),
            WeightedQuery(20, GifQuery::Random("sleep")),
            WeightedQuery(20, GifQuery::Random("dogsleep")),
            WeightedQuery(20, GifQuery::Random("catsleep")),
            WeightedQuery(20, GifQuery::Random("rabbitsleep")),
            WeightedQuery(20, GifQuery::Random("ratsleep")),
            WeightedQuery(20, GifQuery::Random("ducksleep")),
            WeightedQuery(20, GifQuery::Random("animalsleep")),
        ],
        messages: GOOD_NIGHT_WISHES,
    },
};

static GOOD_NIGHT_WISHES: &[&str] = &[
    "Thank you for always being a friend I can count on. Hope you have a great night’s sleep.",
    "Today was the best because I got to spend it with you. Smiling as I fall asleep. Sweet dreams.",
    "Hope you are ending your day with happy thoughts and gratitude, and looking forward to a morning that is as wonderful as you. Good night friend.",
    "I could stay up and talk with you until the sun comes up. Thanks for being the best friend I could ever ask for. Good night.",
    "Hope you fall asleep and dream of the most wonderful things, only to wake up and find them real. Good night.",
    "Before you fall asleep, take a moment to feel gratitude for what a great person you are, and I’ll do the same. Thanks for being the best. Sweet dreams.",
    "I hope you sleep so well tonight. May you wake up to this message in hopes of it bringing a big smile to your face.",
    "Nighty-night! Sleep tight and don't let the bedbugs bite.",
    "Rest easy, friend. Tomorrow's a new day full of possibilities.",
    "Hasta mañana! See you in the morning.",
    "Wrap yourself up in a cozy blanket and drift off to dreamland. Good night!",
    "Savor your rest, you deserve it.",
    "Rest your head on that pillow and let your worries drift away.",
    "Sleep well and dream big.",
    "You're the best! Just wanted to let you know before bed.",
    "Good night, good sleep, good vibes.",
    "Sleep tight and don't forget to set your alarm so you can hit snooze ten times in the morning.",
    "Sleep well and dream of a world where Monday mornings don't exist.",
    "Psst: time to close your eyes!",
    "Good night, sleep well, and remember that tomorrow is another day to procrastinate.",
    "Sleep like a bear in hibernation!",
    r#""Good night, sleep tight. Now the sun turns out his light. Good night, sleep tight, dream sweet dreams for me, dream sweet dreams for you." - The Beatles"#,
    r#""Good night stars, good night air, good night noises everywhere." - Margaret Wise Brown"#,
    r#""Don't fight with the pillow, but lay down your head and kick every worriment out of the bed." - Edmund Vance Cooke"#,
    r#""As the night gets dark, let your worries fade. Sleep peacefully knowing you've done all you can do for today." - Roald Dahl"#,
    r#""A well-spent day brings happy sleep." - Leonardo da Vinci"#,
    r#""Sleep is the best meditation." - Dalai Lama"#,
    r#""There is a time for many words and there is also a time for sleep." —Homer"#,
];

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
struct DayOfMonth(u8, chrono::Month);

#[derive(Debug, Copy, Clone)]
struct DateRange {
    start: DayOfMonth,
    end: DayOfMonth,
}

impl PartialEq<&NaiveDate> for DateRange {
    fn eq(&self, other: &&NaiveDate) -> bool {
        let day = other.day();
        let month = other.month();
        let start_month = self.start.1.number_from_month();
        let end_month = self.end.1.number_from_month();
        (month >= start_month && month <= end_month)
            && !(month == start_month && day < self.start.0 as u32)
            && !(month == end_month && day > self.end.0 as u32)
    }
}

#[derive(Debug, Copy, Clone)]
enum GifQuery {
    Single(&'static str),
    Random(&'static str),
    Search(&'static str),
}

impl GifQuery {
    async fn find(&self, bot: &SpiderBot) -> Result<Cow<'static, str>, GifError> {
        match self {
            GifQuery::Single(url) => Ok(Cow::Borrowed(url)),
            GifQuery::Random(query) | GifQuery::Search(query) => {
                let gif = get_gif(bot, query, matches!(self, GifQuery::Random(_))).await?;
                Ok(Cow::Owned(gif))
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
struct CollectionData {
    queries: &'static [WeightedQuery],
    messages: &'static [&'static str],
}

impl CollectionData {
    async fn find(
        &self,
        bot: &SpiderBot,
    ) -> Result<(Cow<'static, str>, Option<&'static str>), GifError> {
        let dist = WeightedIndex::new(self.queries.iter().map(|q| q.0 as u32))?;
        let query = self.queries[dist.sample(&mut thread_rng())].1;
        let gif = query.find(bot).await?;
        let message = self.messages.choose(&mut thread_rng()).copied();
        Ok((gif, message))
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
    fn current(&self, date: &NaiveDate) -> &CollectionData {
        self.seasons
            .iter()
            .find(|s| s.range == date)
            .map(|s| &s.data)
            .unwrap_or(&self.data)
    }
}
