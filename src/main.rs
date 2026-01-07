extern crate core;

use crate::background_tasks::{DiscordApi, start_cache_trim, start_gif_updater};
use crate::cache::GifCache;
use crate::commands::CommandError;
use crate::commands::gifs::GifError;
use crate::consts::GIF_COUNT;
use db::{BotDatabase, DatabaseConnection};
use dotenv::dotenv;
use poise::CreateReply;
use serenity::all::GatewayIntents;
use serenity::client::Client as Serenity;
use std::env;
use tenor::models::{ContentFilter, MediaFilter};
use tenor::{Client as Tenor, Config};
use tracing_subscriber::prelude::*;

mod background_tasks;
mod cache;
mod commands;
mod consts;
mod context;
mod util;

pub(crate) const BASE_GIF_CONFIG: Config = Config::new()
    .content_filter(ContentFilter::Medium)
    .media_filter(&[MediaFilter::Gif])
    .limit(GIF_COUNT);

#[derive(Debug, Clone)]
struct SpiderBot<'tenor> {
    gif_cache: GifCache,
    tenor: Tenor<'tenor>,
    database: BotDatabase,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let discord_token: &str = env::var("DISCORD_TOKEN")?.leak();
    let tenor_token: &str = env::var("TENOR_TOKEN")?.leak();

    let anime_url = match resolve_env("ANIME_URL") {
        Ok(anime_url) => Some(anime_url.leak()),
        Err(error) => {
            tracing::warn!("Failed to resolve ANIME_URL: {error}");
            None
        }
    };

    let database = db::connect(env!("CARGO_PKG_NAME")).await?;
    database.migrate().await?;

    // Login with a bot token from the environment
    let bot = SpiderBot {
        gif_cache: GifCache::new(),
        tenor: Tenor::with_config(tenor_token, Some(BASE_GIF_CONFIG)),
        database: database.clone(),
    };

    start_gif_updater(bot.tenor.clone(), bot.gif_cache.clone())?;
    start_cache_trim(bot.gif_cache.clone());

    let intents = GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::version(),
                commands::gifs::hurry(),
                commands::gifs::morbin(),
                commands::gifs::play(),
                commands::gifs::sleep(),
                commands::true_coin::coin(),
            ],
            on_error: |error| {
                Box::pin(async move {
                    if let Err(e) = on_error(error).await {
                        tracing::error!("Error while handling error: {}", e);
                    }
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(bot)
            })
        })
        .build();

    let mut client = Serenity::builder(discord_token, intents)
        .framework(framework)
        .await?;

    if let Some(anime_url) = anime_url {
        DiscordApi::from(&client).publish_anime_updates(database, anime_url);
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.shutdown_all().await;
    });

    // start listening for events by starting a single shard
    client.start().await?;

    Ok(())
}

fn resolve_env(key: &str) -> anyhow::Result<String> {
    use envmnt::{ExpandOptions, ExpansionType};
    let key = env::var(key)?;
    let options = ExpandOptions {
        expansion_type: Some(ExpansionType::All),
        default_to_empty: true,
    };
    Ok(envmnt::expand(&key, Some(options)))
}

async fn on_error(
    error: poise::FrameworkError<'_, SpiderBot<'_>, CommandError>,
) -> Result<(), serenity::Error> {
    match error {
        poise::FrameworkError::Command { ctx, error, .. } => {
            let error_message = match error {
                CommandError::GifError(GifError::NoGifs | GifError::RestrictedQuery(_)) => {
                    error.to_string()
                }
                _ => "Internal error".to_string(),
            };
            eprintln!("An error occurred in a command: {error}");
            let msg = CreateReply::default()
                .ephemeral(true)
                .content(error_message);
            ctx.send(msg).await?;
            Ok(())
        }
        error => poise::builtins::on_error(error).await,
    }
}
