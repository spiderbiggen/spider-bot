#[macro_use]
extern crate diesel;
extern crate dotenv;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Duration, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    Args, CommandGroup, CommandResult, help_commands, HelpOptions,
    macros::{
        command,
        group,
        help,
    },
    StandardFramework,
};
use serenity::model::channel::Message;
use serenity::model::id::{ChannelId, GuildId};
use serenity::model::prelude::UserId;
use tokio::time;
use tokio::time::Instant;

use commands::{
    dice::*,
    gifs::*
};
use kitsu::api::anime as anime_api;
use models::Subscription;
use nyaa::Anime;
use crate::anime::AnimeGroup;

pub mod schema;
pub mod models;
mod anime;
mod commands;

lazy_static! {
    static ref BOT_ID: UserId = UserId(env::var("BOT_ID").map(|a| a.parse::<u64>().expect("BOT_ID was not a number")).expect("BOT_ID was not set or not a number"));
}

pub fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

#[help]
#[individual_command_tip =
"Hello! こんにちは！Hola! Bonjour! 您好! 안녕하세요~\n\n\
If you want more information about a specific command, just pass the command as argument."]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[lacking_permissions = "Hide"]
#[lacking_role = "Nothing"]
#[wrong_channel = "Strike"]
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
#[commands(ping, roll, night)]
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

        // it's safe to clone Context, but Arc is cheaper for this use case.
        // Untested claim, just theoretically. :P
        let ctx = Arc::new(ctx);
        let guilds = Arc::new(guilds);

        // We need to check that the loop is not already running when this event triggers,
        // as this event triggers every time the bot enters or leaves a guild, along every time the
        // ready shard event triggers.
        //
        // An AtomicBool is used because it doesn't require a mutable reference to be changed, as
        // we don't have one due to self being an immutable reference.
        if !self.is_loop_running.load(Ordering::Relaxed) {

            // We have to clone the Arc, as it gets moved into the new thread.
            let ctx1 = Arc::clone(&ctx);
            let guilds1 = Arc::clone(&guilds);
            // tokio::spawn creates a new green thread that can run in parallel with the rest of
            // the application.
            tokio::spawn(periodic_fetch(ctx1, guilds1));
            // Now that the loop is running, we set the bool to true
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("/").on_mention(Some(*BOT_ID)).no_dm_prefix(true)) // set the bot's prefix to "/"
        .group(&GENERAL_GROUP)
        .help(&MY_HELP);

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let _database_url = env::var("DATABASE_URL").expect("database");
    let mut client = Client::builder(token)
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false),
        })
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

async fn periodic_fetch(context: Arc<Context>, guilds: Arc<Vec<GuildId>>) {
    let mut last = Instant::now().checked_sub(std::time::Duration::from_secs(1800)).unwrap_or(Instant::now());
    // let mut last = Instant::now().checked_sub(std::time::Duration::from_secs(3600)).unwrap_or(Instant::now());
    let mut interval_day = time::interval(std::time::Duration::from_secs(600));
    loop {
        let now = interval_day.tick().await;
        let prev_time = Utc::now().checked_sub_signed(Duration::from_std(now.duration_since(last)).unwrap()).unwrap();

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

async fn send_anime_embed(ctx: &Arc<Context>, channel: &ChannelId, group: &AnimeGroup, anime: &Vec<Anime>) {
    if let Err(why) = channel.send_message(&ctx, |m| m.embed(|e| {
        e.title(format!("{} Ep {}", &group.title, group.episode.map_or("".to_string(), |a| a.to_string())));
        anime.into_iter().for_each(|anime| {
            e.field(
                &anime.resolution,
                format!("[torrent]({})\n[comments]({})", &anime.torrent, &anime.comments),
                true,
            );
        });
        e
    })).await {
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
        map.insert("Kumo desu ga, Nani ka".to_string(), vec![
            ChannelId(825808364649971715),
        ]);
        map
    });
    map.insert(GuildId(165162546444107776), {
        let mut map: HashMap<String, Vec<ChannelId>> = HashMap::new();
        map.insert("Boku no Hero Academia".to_string(), vec![
            ChannelId(178167855718727680),
        ]);
        map
    });

    map
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}
