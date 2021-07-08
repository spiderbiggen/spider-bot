#[cfg(feature = "giphy")]
pub mod giphy {
    use giphy::models::ContentFilter;
    use giphy::Client;
    use serenity::client::Context;
    use serenity::framework::standard::{
        macros::{command, group},
        CommandResult,
    };
    use serenity::model::channel::Message;
    use std::env;

    #[command]
    pub async fn night(ctx: &Context, msg: &Message) -> CommandResult {
        let token = env::var("GIPHY_TOKEN")?;
        let client = Client::new(token, Some(ContentFilter::Off));
        let single = client.random("good night").await?;
        msg.reply(ctx, single.embed_url.as_str()).await?;
        Ok(())
    }

    #[command]
    pub async fn sleep(ctx: &Context, msg: &Message) -> CommandResult {
        let token = env::var("GIPHY_TOKEN")?;
        let client = Client::new(token, Some(ContentFilter::Off));
        let single = client.random("sleep well").await.unwrap();
        msg.reply(ctx, single.embed_url.as_str()).await?;
        Ok(())
    }

    #[group]
    #[prefixes("g", "giphy")]
    #[commands(sleep, night)]
    pub struct Giphy;
}

#[cfg(feature = "tenor")]
pub mod tenor {
    use rand::seq::SliceRandom;
    use serenity::client::Context;
    use serenity::framework::standard::{
        macros::{command, group},
        CommandResult,
    };
    use serenity::model::channel::Message;
    use std::env;
    use tenor::models::ContentFilter;
    use tenor::Client;

    #[command]
    pub async fn night(ctx: &Context, msg: &Message) -> CommandResult {
        let token = env::var("TENOR_TOKEN")?;
        let client = Client::new(token, Some(ContentFilter::Off));
        let results = client.random("good night").await.unwrap();
        let single = results.choose(&mut rand::thread_rng()).unwrap();
        msg.reply(ctx, single.url.as_str()).await?;
        Ok(())
    }

    #[command]
    pub async fn sleep(ctx: &Context, msg: &Message) -> CommandResult {
        let token = env::var("TENOR_TOKEN")?;
        let client = Client::new(token, Some(ContentFilter::Off));
        let results = client.random("sleep well").await.unwrap();
        let single = results.choose(&mut rand::thread_rng()).unwrap();
        msg.reply(ctx, single.url.as_str()).await?;
        Ok(())
    }

    #[group]
    #[prefixes("t", "tenor")]
    #[commands(sleep, night)]
    pub(crate) struct Tenor;
}

pub(crate) struct Gifs;
