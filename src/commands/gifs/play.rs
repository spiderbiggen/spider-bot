use super::refresh_gif_cache_for_query;
use crate::Tenor;
use crate::cache::GifCache;
use crate::commands::gifs::{GifError, MAX_AUTOCOMPLETE_RESULTS, get_cached_gif};
use crate::context::{Context, GifContextExt};
use rustrict::CensorStr;
use std::borrow::Cow;
use std::fmt::Write;
use std::sync::Arc;
use tenor::Config;
use url::Url;

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
        query: "apex legends",
        matches: &["apex legends"],
    },
    GameQuery {
        name: "Battlefield",
        query: "battlefield",
        matches: &["battlefield"],
    },
    GameQuery {
        name: "Call of Duty",
        query: "call of duty",
        matches: &["warzone", "cod", "call of duty"],
    },
    GameQuery {
        name: "Chivalry 2",
        query: "chivalry 2",
        matches: &["chivalry 2"],
    },
    GameQuery {
        name: "Halo Infinite",
        query: "halo",
        matches: &["halo infinite"],
    },
    GameQuery {
        name: "League of Legends",
        query: "league of legends",
        matches: &["lol", "league of legends"],
    },
    GameQuery {
        name: "Lethal Company",
        query: "lethal company",
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
        name: "Sid Meier's Civilization VII",
        query: "sid meier's civilization",
        matches: &[
            "civilization",
            "sid meier's civilization vii",
            "sid meier's civilization 7",
        ],
    },
];

pub struct CommandOutput {
    pub message: String,
    pub gif: Arc<Url>,
}

// TODO improve autocomplete by checking another service instead of a static list
#[expect(clippy::unused_async)]
pub async fn autocomplete(_: Context<'_, '_>, partial: &str) -> Vec<Cow<'static, str>> {
    let lower_partial = &partial.to_lowercase();
    GAME_AUTOCOMPLETION
        .iter()
        .filter(|GameQuery { matches, .. }| matches.iter().any(|s| s.starts_with(lower_partial)))
        .map(|&GameQuery { name, .. }| Cow::Borrowed(name))
        .take(MAX_AUTOCOMPLETE_RESULTS)
        .collect()
}

pub async fn get_command_output(
    context: &impl GifContextExt<'_>,
    mention: &str,
    game: Option<String>,
) -> Result<CommandOutput, GifError> {
    let gif_cache = context.gif_cache();
    let gif = match &game {
        None => get_cached_gif(gif_cache, PLAY_FALLBACK)?,
        Some(game) => get_game_gif(context.tenor(), gif_cache, game).await?,
    };
    let mut message = format!("{mention}! Let's play ");
    if let Some(game) = game {
        write!(message, "some {game}!").expect("writing to string should not fail");
    } else {
        write!(message, "a game!").expect("writing to string should not fail");
    }
    Ok(CommandOutput { message, gif })
}

async fn get_game_gif(
    tenor: &Tenor<'_>,
    gif_cache: &GifCache,
    game: &str,
) -> Result<Arc<Url>, GifError> {
    let query = transform_query(game)?;
    match get_cached_gif(gif_cache, &query) {
        Ok(gif) => Ok(gif),
        Err(GifError::NoGifs) => {
            if refresh_gif_cache_for_query(tenor, gif_cache, &query, None).await {
                get_cached_gif(gif_cache, &query)
            } else {
                Err(GifError::NoGifs)
            }
        }
        Err(err) => Err(err),
    }
}

#[tracing::instrument(skip_all)]
pub async fn refresh_play_gifs(tenor: &Tenor<'_>, gif_cache: &GifCache) {
    refresh_gif_cache_for_query(tenor, gif_cache, PLAY_FALLBACK, Some(FALLBACK_CONFIG)).await;

    // TODO cache n most popular games
    for &GameQuery { query, .. } in GAME_AUTOCOMPLETION {
        refresh_gif_cache_for_query(tenor, gif_cache, query, None).await;
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
