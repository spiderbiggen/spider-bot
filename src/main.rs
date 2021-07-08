#[macro_use]
extern crate diesel;
extern crate dotenv;
#[macro_use]
extern crate anyhow;

mod anime;
mod commands;
pub mod models;
pub mod schema;
#[cfg(feature = "anime_storage")]
mod storage;
mod util;

use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use dotenv::dotenv;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    help_commands,
    macros::{command, group, help},
    Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::http::Http;
use serenity::model::{
    channel::Message,
    id::{ChannelId, GuildId},
    prelude::UserId,
};
use tokio::time;
use tokio::time::Instant;

#[cfg(feature = "giphy")]
use commands::gifs::giphy::GIPHY_GROUP;
#[cfg(feature = "tenor")]
use commands::gifs::tenor::TENOR_GROUP;
use commands::{anime::*, dice::*};
use nyaa::Anime;

use crate::anime::AnimeGroup;

#[help]
#[max_levenshtein_distance(3)]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[group]
#[commands(ping, roll, search)]
struct General;

struct Handler {
    is_loop_running: AtomicBool,
}

#[async_trait]
impl EventHandler for Handler {
    // We use the cache_ready event just in case some cache operation is required in whatever use
    // case you have for this.
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);
        let guilds = Arc::new(guilds);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            // We have to clone the Arc, as it gets moved into the new thread.
            let ctx1 = Arc::clone(&ctx);
            let guilds1 = Arc::clone(&guilds);
            tokio::spawn(periodic_fetch(ctx1, guilds1));
            // Now that the loop is running, we set the bool to true
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

async fn get_bot_info(token: &str) -> Result<(HashSet<UserId>, UserId)> {
    let http = Http::new_with_token(token.as_ref());

    // We will fetch your bot's owners and id
    let info = http.get_current_application_info().await?;
    let mut owners = HashSet::new();
    match info.team {
        Some(team) => owners.insert(team.owner_user_id),
        None => owners.insert(info.owner.id),
    };
    match http.get_current_user().await {
        Ok(bot_id) => Ok((owners, bot_id.id)),
        Err(why) => Err(anyhow!("Could not access the bot id: {}", why)),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN").expect("token");

    // Login with a bot token from the environment
    let (owners, bot_id) = get_bot_info(&token).await?;

    let mut framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .prefix("/")
                .on_mention(Some(bot_id))
                .no_dm_prefix(true)
                .owners(owners)
        }) // set the bot's prefix to "/"
        .group(&GENERAL_GROUP)
        .help(&MY_HELP);

    if cfg!(feature = "giphy") {
        framework = framework.group(&GIPHY_GROUP);
    }
    if cfg!(feature = "tenor") {
        framework = framework.group(&TENOR_GROUP);
    }

    let mut client = Client::builder(token)
        .event_handler(Handler {
            is_loop_running: false.into(),
        })
        .application_id(bot_id.0)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
    Ok(())
}

async fn periodic_fetch(context: Arc<Context>, guilds: Arc<Vec<GuildId>>) {
    let mut last = Instant::now()
        .checked_sub(std::time::Duration::from_secs(1800))
        .unwrap_or(Instant::now());
    // let mut last = Instant::now().checked_sub(std::time::Duration::from_secs(3600)).unwrap_or(Instant::now());
    let mut interval_day = time::interval(std::time::Duration::from_secs(600));
    loop {
        let now = interval_day.tick().await;
        let prev_time = Utc::now()
            .checked_sub_signed(Duration::from_std(now.duration_since(last)).unwrap())
            .unwrap();

        let groups = anime::update_from_nyaa(prev_time).await;
        let subscriptions = get_subscriptions_for_channel().await;

        for (group, anime) in groups {
            for guild in guilds.iter() {
                if let Some(sub) = subscriptions.get(guild) {
                    if let Some(channels) = sub.get(&group.title) {
                        for channel in channels {
                            send_anime_embed(&context, channel, &group, &anime).await;
                        }
                    }
                }
            }
        }
        last = now;
    }
}

async fn send_anime_embed(
    ctx: &Arc<Context>,
    channel: &ChannelId,
    group: &AnimeGroup,
    anime: &Vec<Anime>,
) {
    if let Err(why) = channel
        .send_message(&ctx, |m| {
            m.embed(|e| {
                e.title(format!(
                    "{} Ep {}",
                    &group.title,
                    group.episode.map_or("".to_string(), |a| a.to_string())
                ));
                anime.into_iter().for_each(|anime| {
                    e.field(
                        &anime.resolution,
                        format!(
                            "[torrent]({})\n[comments]({})",
                            &anime.torrent, &anime.comments
                        ),
                        true,
                    );
                });
                e
            })
        })
        .await
    {
        eprintln!("Error sending message: {:?}", why);
    };
}

async fn get_subscriptions_for_channel() -> HashMap<GuildId, HashMap<String, Vec<ChannelId>>> {
    let mut map: HashMap<GuildId, HashMap<String, Vec<ChannelId>>> = HashMap::new();

    // map.insert(GuildId(165162546444107776), vec![
    //
    // ]);
    map.insert(GuildId(825808364649971712), {
        let mut map: HashMap<String, Vec<ChannelId>> = HashMap::new();
        map.insert(
            "Kumo desu ga, Nani ka".to_string(),
            vec![ChannelId(825808364649971715)],
        );
        map
    });
    map.insert(GuildId(165162546444107776), {
        let mut map: HashMap<String, Vec<ChannelId>> = HashMap::new();
        map.insert(
            "Boku no Hero Academia".to_string(),
            vec![ChannelId(178167855718727680)],
        );
        map
    });

    map
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}
