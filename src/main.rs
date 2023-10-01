#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dotenv::dotenv;
use itertools::Itertools;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::gateway::Ready;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::GuildId;
use serenity::model::prelude::{Interaction, ResumedEvent};
use serenity::prelude::GatewayIntents;
use serenity::prelude::*;
use tracing::{error, info};
use tracing_subscriber::prelude::*;

use tenor::models::Gif;

use crate::background_tasks::run_periodic_tasks;

mod background_tasks;
mod cache;
mod commands;
mod consts;
mod util;

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

#[derive(Debug, Clone)]
struct SpiderBot {
    gif_cache: cache::Memory<[Gif]>,
    tenor: tenor::Client,
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
            run_periodic_tasks(self);
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
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

    let discord_token = Box::leak(Box::new(env::var("DISCORD_TOKEN")?));
    let tenor_token = env::var("TENOR_TOKEN")?;

    // Login with a bot token from the environment
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(discord_token, intents)
        .event_handler(SpiderBot {
            gif_cache: cache::Memory::new(),
            tenor: tenor::Client::new(tenor_token),
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
    if let Err(err) = client.start().await {
        error!(error = ?err, "An error occurred while running the client");
    }
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
