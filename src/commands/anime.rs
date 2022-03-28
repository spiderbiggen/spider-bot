use std::error::Error;

use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;

use kitsu::api;
use kitsu::models::Anime;

use crate::util::edit_distance;

pub(crate) async fn get_anime<S: AsRef<str>>(title: S) -> Result<Vec<Anime>, Box<dyn Error>> {
    let mut anime = api::anime::get_collection(&title).await?;
    anime.sort_by_key(|a| edit_distance(&title, &a.canonical_title));
    Ok(anime)
}

#[command]
async fn search(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let title = args.message();
    let message = if !title.is_empty() {
        let results = get_anime(title).await;
        match results {
            Ok(results) if !results.is_empty() => results
                .into_iter()
                .map(|a| format!("{} <{}>", a.canonical_title, a.rating.unwrap_or("?".into())))
                .collect::<Vec<String>>()
                .join("\n"),
            _ => format!("No results found for {}", title),
        }
    } else {
        "Give a title as argument".into()
    };
    msg.reply(ctx, message).await?;
    Ok(())
}
