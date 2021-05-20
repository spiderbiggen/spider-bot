use serenity::client::Context;
use serenity::framework::standard::{Args, CommandResult, macros::command};
use serenity::model::channel::Message;
use giphy::Client as Giphy;
use tenor::Client as Tenor;
use std::env;
use rand::seq::SliceRandom;


#[command]
pub async fn night(ctx: &Context, msg: &Message) -> CommandResult {
    let token = env::var("GIPHY_TOKEN")?;
    let client = Giphy::new(token);
    let single = client.random("good night").await.unwrap();
    msg.reply(ctx, single.embed_url.as_str()).await?;
    Ok(())
}

#[command]
pub async fn night2(ctx: &Context, msg: &Message) -> CommandResult {
    let token = env::var("TENOR_TOKEN")?;
    let client = Tenor::new(token);
    let results = client.random("good night").await.unwrap();
    let single = results.choose(&mut rand::thread_rng()).unwrap();
    msg.reply(ctx, single.url.as_str()).await?;
    Ok(())
}

#[command]
pub async fn sleep(ctx: &Context, msg: &Message) -> CommandResult {
    let token = env::var("GIPHY_TOKEN")?;
    let client = Giphy::new(token);
    let single = client.random("sleep well").await.unwrap();
    msg.reply(ctx, single.embed_url.as_str()).await?;
    Ok(())
}

#[command]
pub async fn sleep2(ctx: &Context, msg: &Message) -> CommandResult {
    let token = env::var("TENOR_TOKEN")?;
    let client = Tenor::new(token);
    let results = client.random("sleep well").await.unwrap();
    let single = results.choose(&mut rand::thread_rng()).unwrap();
    msg.reply(ctx, single.url.as_str()).await?;
    Ok(())
}