#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dotenv::dotenv;
use itertools::Itertools;
use serenity::client::bridge::gateway::ShardManager;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::gateway::Ready;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::GuildId;
use serenity::model::prelude::Interaction;
use serenity::prelude::GatewayIntents;
use serenity::prelude::*;
use serenity::{async_trait, CacheAndHttp};
use tracing::{error, info};
use tracing_subscriber::prelude::*;

use tenor::models::Gif;

use crate::background_tasks::start_background_tasks;

mod background_tasks;
mod cache;
mod commands;
mod consts;
mod messaging;
mod util;

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

#[derive(Debug, Clone)]
struct Config {
    anime_url: &'static str,
}

#[derive(Debug, Clone)]
struct SpiderBot {
    config: Config,
    gif_cache: cache::Memory<[Gif]>,
    tenor: tenor::Client,
    pool: otaku::db::Pool,
    is_loop_running: Arc<AtomicBool>,
}

#[async_trait]
impl EventHandler for SpiderBot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
        if cfg!(debug_assertions) {
            for guild in ready.guilds {
                log_slash_commands(
                    guild
                        .id
                        .set_application_commands(&ctx, |bot_commands| {
                            commands::register_commands(bot_commands)
                        })
                        .await,
                    Some(guild.id),
                );
            }
        } else {
            log_slash_commands(
                Command::set_global_application_commands(&ctx, |bot_commands| {
                    commands::register_commands(bot_commands)
                })
                .await,
                None,
            );
        }

        if !self.is_loop_running.load(Ordering::Relaxed) {
            start_background_tasks(self, ctx);
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {
                commands::interaction(command, &ctx, self).await;
            }
            Interaction::Autocomplete(command) => {
                commands::autocomplete(command, &ctx).await;
            }
            _ => {
                error!("Unsupported interaction type received: {:?}", interaction);
            }
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
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(discord_token, intents)
        .event_handler(SpiderBot {
            config: Config { anime_url },
            gif_cache: cache::Memory::new(),
            tenor: tenor::Client::new(tenor_token),
            pool,
            is_loop_running: Arc::new(AtomicBool::new(false)),
        })
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.lock().await.shutdown_all().await;
    });

    // start listening for events by starting a single shard
    client.start().await?;

    #[cfg(debug_assertions)]
    remove_guild_commands(client.cache_and_http).await;

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
async fn remove_guild_commands(cache_and_http: Arc<CacheAndHttp>) {
    let cache = cache_and_http.cache.clone();
    let http = cache_and_http.http.clone();
    for guild in cache.guilds() {
        let command_ids = guild.get_application_commands(&http).await;
        match command_ids {
            Ok(ids) => {
                for command in ids {
                    let result = guild.delete_application_command(&http, command.id).await;
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
