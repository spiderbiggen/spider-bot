extern crate core;

use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::sync::atomic::AtomicBool;
#[cfg(feature = "nyaa")]
use std::sync::atomic::Ordering;
#[cfg(feature = "nyaa")]
use std::sync::Arc;

use dotenv::dotenv;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    help_commands,
    macros::{command, group, help},
    Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::http::Http;
use serenity::model::{channel::Message, id::GuildId, prelude::UserId};

#[cfg(feature = "kitsu")]
use commands::anime::*;
use commands::dice::*;
use commands::gifs::giphy::GIPHY_GROUP;
use commands::gifs::tenor::TENOR_GROUP;

mod commands;
#[cfg(feature = "nyaa")]
mod nyaa;
#[cfg(feature = "kitsu")]
mod util;

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

#[cfg(feature = "kitsu")]
#[group]
#[commands(ping, roll, search)]
struct General;

#[cfg(not(feature = "kitsu"))]
#[group]
#[commands(ping, roll)]
struct General;

struct Handler {
    is_loop_running: AtomicBool,
}

#[async_trait]
impl EventHandler for Handler {
    // We use the cache_ready event just in case some cache operation is required in whatever use
    // case you have for this.
    #[cfg(feature = "nyaa")]
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);
        let guilds = Arc::new(guilds);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            // We have to clone the Arc, as it gets moved into the new thread.
            let ctx1 = Arc::clone(&ctx);
            let guilds1 = Arc::clone(&guilds);
            tokio::spawn(nyaa::periodic_fetch(ctx1, guilds1));
            // Now that the loop is running, we set the bool to true
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

async fn get_bot_info(token: &str) -> Result<(HashSet<UserId>, UserId), Box<dyn Error>> {
    let http = Http::new_with_token(token.as_ref());

    // We will fetch your bot's owners and id
    let info = http.get_current_application_info().await?;
    let mut owners = HashSet::new();
    match info.team {
        Some(team) => owners.insert(team.owner_user_id),
        None => owners.insert(info.owner.id),
    };
    Ok(http
        .get_current_user()
        .await
        .map(move |user| (owners, user.id))?)
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN").expect("token");

    // Login with a bot token from the environment
    let (owners, bot_id) = match get_bot_info(&token).await {
        Ok((owners, id)) => (owners, id),
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

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
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}
