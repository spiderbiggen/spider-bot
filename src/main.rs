use std::env;
use std::sync::Arc;

use dotenv::dotenv;
use itertools::Itertools;
use serenity::all::{
    Cache, Command, GatewayIntents, GuildId, Http, Interaction, Ready, ShardManager,
};
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::prelude::*;
use tracing::{error, info};
use tracing_subscriber::prelude::*;

use tenor::models::Gif;

use crate::background_tasks::{start_anime_subscription, start_cache_trim};

mod background_tasks;
mod cache;
mod commands;
mod consts;
mod messaging;
mod util;

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<ShardManager>;
}

#[derive(Debug, Clone)]
struct SpiderBot {
    gif_cache: cache::Memory<[Gif]>,
    tenor: tenor::Client,
    pool: otaku::db::Pool,
}

#[async_trait]
impl EventHandler for SpiderBot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
        let mut create_commands = vec![];
        commands::register_commands(&mut create_commands);
        if cfg!(debug_assertions) {
            for guild_id in ready.guilds.iter().map(|guild| guild.id) {
                log_slash_commands(
                    guild_id.set_commands(&ctx, create_commands.clone()).await,
                    Some(guild_id),
                );
            }
        } else {
            log_slash_commands(
                Command::set_global_commands(&ctx, create_commands).await,
                None,
            );
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => commands::interaction(command, &ctx, self).await,
            Interaction::Autocomplete(command) => commands::autocomplete(command, &ctx).await,
            _ => error!("Unsupported interaction type received: {:?}", interaction),
        }
    }
}

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
        pool,
    };

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(discord_token, intents)
        .event_handler(bot.clone())
        .await?;

    start_cache_trim(bot.gif_cache.clone());
    start_anime_subscription(
        bot.pool,
        anime_url,
        client.cache.clone(),
        client.http.clone(),
    );

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
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

    #[cfg(debug_assertions)]
    remove_guild_commands(&client.cache, &client.http).await;

    Ok(())
}

fn log_slash_commands(result: serenity::Result<Vec<Command>>, guild_id: Option<GuildId>) {
    match (result, guild_id) {
        (Ok(c), _) => {
            let commands_registered = c.iter().map(|cmd| &cmd.name).join(", ");
            info!("Commands registered: {commands_registered}");
        }
        (Err(e), Some(guild)) => error!("Error setting slash commands for guild {guild}: {e}"),
        (Err(e), None) => error!("Error setting global slash commands: {e}"),
    };
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

#[cfg(debug_assertions)]
async fn remove_guild_commands(cache: &Cache, http: &Http) {
    for guild in cache.guilds() {
        let command_ids = guild.get_commands(&http).await;
        match command_ids {
            Ok(ids) => {
                for command in ids {
                    let result = guild.delete_command(&http, command.id).await;
                    if let Err(err) = result {
                        error!(
                            "Could not delete command {} for guild {guild}: {err:?}",
                            command.name
                        );
                    }
                }
            }
            Err(err) => {
                error!("Could not get command ids for guild {guild}: {err:?}");
            }
        }
    }
}
