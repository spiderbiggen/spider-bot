use super::{cache_gifs, update_cached_gifs, GifSliceExt};
use crate::commands::gifs::{get_cached_gif, GifError, MAX_AUTOCOMPLETE_RESULTS};
use crate::consts::LONG_CACHE_LIFETIME;
use crate::context::GifContextExt;
use futures::{Stream, StreamExt};
use rustrict::CensorStr;
use std::borrow::Cow;
use tenor::Config;
use tracing::error;

const FALLBACK_CONFIG: Config = super::RANDOM_CONFIG;
static PLAY_FALLBACK: &str = "games";

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
    context: &impl GifContextExt<'_>,
    mention: &str,
    game: Option<String>,
) -> Result<CommandOutput, GifError> {
    let gif = match &game {
        None => get_cached_gif(context, PLAY_FALLBACK).await?,
        Some(game) => {
            let query = transform_query(game)?;
            match get_cached_gif(context, &query).await {
                Ok(gif) => gif,
                Err(GifError::NoGifs) => {
                    let gifs = update_cached_gifs(context, query.clone(), None).await?;
                    gifs.take()?
                }
                Err(err) => Err(err)?,
            }
        }
    };
    let message = if let Some(game) = &game {
        format!("{mention}! Let's play some {game}!")
    } else {
        format!("{mention}! Let's play a game!")
    };
    Ok(CommandOutput { message, gif })
}

pub async fn update_gif_cache(context: &impl GifContextExt<'_>) {
    let tenor = context.tenor();
    for GameQuery { query, .. } in GAME_AUTOCOMPLETION {
        match tenor.search(query, None).await {
            Ok(gifs) => {
                cache_gifs(context, *query, gifs, LONG_CACHE_LIFETIME).await;
            }
            Err(error) => error!("Error caching gifs for {query}: {error}"),
        };
    }
    match tenor.search(PLAY_FALLBACK, Some(FALLBACK_CONFIG)).await {
        Ok(gifs) => {
            cache_gifs(context, PLAY_FALLBACK, gifs, LONG_CACHE_LIFETIME).await;
        }
        Err(error) => error!("Error caching gifs for {PLAY_FALLBACK}: {error}"),
    };
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
