use serenity::client::Context;
use serenity::framework::standard::{Args, CommandResult, macros::command};
use serenity::model::channel::Message;
use giphy::Client;
use std::env;
use rand::seq::SliceRandom;


#[command]
pub async fn night(ctx: &Context, msg: &Message) -> CommandResult {
    let token = env::var("GIPHY_TOKEN")?;
    let client = Client::new(token);
    let single = client.random("good night").await.unwrap();
    msg.reply(ctx, single.embed_url.as_str()).await?;
    Ok(())
}