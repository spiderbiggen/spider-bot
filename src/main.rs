use std::env;

use dotenv::dotenv;
use serenity::all::GatewayIntents;
use serenity::client::Client;
use tracing_subscriber::prelude::*;
use url::Url;

use crate::background_tasks::{
    start_anime_subscription, start_cache_trim, start_sleep_gif_updater,
};
use crate::commands::CommandError;

mod background_tasks;
mod cache;
mod commands;
mod consts;

#[derive(Debug, Clone)]
struct SpiderBot<'tenor_config> {
    gif_cache: cache::Memory<[Url]>,
    tenor: tenor::Client<'tenor_config>,
}

type Context<'a, 'tenor_config> = poise::Context<'a, SpiderBot<'tenor_config>, CommandError>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let discord_token = env::var("DISCORD_TOKEN")?.leak();
    let anime_url = resolve_env("ANIME_URL")?.leak();
    let tenor_token = env::var("TENOR_TOKEN")?;

    let pool = otaku::db::connect(env!("CARGO_PKG_NAME")).await?;
    otaku::db::migrate(&pool).await?;
    // Login with a bot token from the environment
    let bot = SpiderBot {
        gif_cache: cache::Memory::new(),
        tenor: tenor::Client::new(tenor_token),
    };

    start_sleep_gif_updater(bot.tenor.clone(), bot.gif_cache.clone())?;
    start_cache_trim(bot.gif_cache.clone());

    let intents = GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::gifs::hurry(),
                commands::gifs::morbin(),
                commands::gifs::play(),
                commands::gifs::sleep(),
            ],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(bot)
            })
        })
        .build();

    let mut client = Client::builder(discord_token, intents)
        .framework(framework)
        .await?;

    start_anime_subscription(pool, anime_url, client.cache.clone(), client.http.clone());

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
