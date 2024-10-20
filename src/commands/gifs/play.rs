use crate::cache;
use crate::commands::gifs::{GifError, MAX_AUTOCOMPLETE_RESULTS};
use futures::{Stream, StreamExt};
use rustrict::CensorStr;
use std::borrow::Cow;
use tracing::error;
use url::Url;

struct GameQuery {
    name: &'static str,
    query: &'static str,
    matches: &'static [&'static str],
}

static GAME_AUTOCOMPLETION: &[GameQuery] = &[
    GameQuery {
        name: "Apex Legends",
        query: "apex_legends",
        matches: &["apex legends"],
    },
    GameQuery {
        name: "Call of Duty",
        query: "call_of_duty",
        matches: &["warzone", "cod", "call of duty"],
    },
    GameQuery {
        name: "Chivalry 2",
        query: "chivalry_2",
        matches: &["chivalry 2"],
    },
    GameQuery {
        name: "Halo Infinite",
        query: "halo",
        matches: &["halo infinite"],
    },
    GameQuery {
        name: "League of Legends",
        query: "league_of_legends",
        matches: &["lol", "league of legends"],
    },
    GameQuery {
        name: "Lethal Company",
        query: "lethal_company",
        matches: &["lethal company"],
    },
    GameQuery {
        name: "Overwatch 2",
        query: "overwatch",
        matches: &["overwatch", "ow"],
    },
    GameQuery {
        name: "Phasmophobia",
        query: "phasmophobia",
        matches: &["phasmophobia"],
    },
    GameQuery {
        name: "Rimworld",
        query: "rimworld",
        matches: &["rimworld"],
    },
    GameQuery {
        name: "Sid Meier's Civilization VI",
        query: "civilization",
        matches: &["civilization", "sid meier's civilization vi"],
    },
];

pub struct CommandOutput {
    pub message: String,
    pub gif: String,
}

pub fn autocomplete(partial: &str) -> impl Stream<Item = &'static str> + '_ {
    futures::stream::iter(GAME_AUTOCOMPLETION)
        .filter(move |GameQuery { matches, .. }| {
            futures::future::ready(matches.iter().any(|s| s.starts_with(partial)))
        })
        .map(|&GameQuery { name, .. }| futures::future::ready(name))
        .buffered(MAX_AUTOCOMPLETE_RESULTS)
        .take(MAX_AUTOCOMPLETE_RESULTS)
}

pub async fn get_command_output(
    tenor: &tenor::Client<'_>,
    gif_cache: &cache::Memory<[Url]>,
    mention: &str,
    game: Option<String>,
) -> Result<CommandOutput, GifError> {
    let query = match &game {
        None => Cow::Borrowed("games"),
        Some(game) => transform_query(game)?,
    };
    let gif = super::get_gif(tenor, gif_cache, query, false).await?;
    let message = if let Some(game) = &game {
        format!("{mention}! Let's play some {game}!")
    } else {
        format!("{mention}! Let's play a game!")
    };
    Ok(CommandOutput { message, gif })
}

pub async fn update_gif_cache(tenor: &tenor::Client<'_>, gif_cache: &cache::Memory<[Url]>) {
    for GameQuery { query, .. } in GAME_AUTOCOMPLETION {
        if let Err(error) = super::cache_gifs(tenor, gif_cache, Cow::Borrowed(query), false).await {
            error!("Error caching gifs for {query}: {error}");
        }
    }
}

fn transform_query(input: &str) -> Result<Cow<'static, str>, GifError> {
    let query = GAME_AUTOCOMPLETION
        .iter()
        .find(|GameQuery { name, .. }| name == &input);
    match query {
        Some(GameQuery { query, .. }) => Ok(Cow::Borrowed(query)),
        None if input.is_inappropriate() => Err(GifError::RestrictedQuery(input.to_string())),
        None => Ok(Cow::Owned(transform_game_to_gif_query(input))),
    }
}

fn transform_game_to_gif_query(game: &str) -> String {
    game.to_lowercase().replace(' ', "_")
}
