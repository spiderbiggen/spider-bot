use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;

use crate::anime;

#[command]
async fn search(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let title = args.message();
    let message = if !title.is_empty() {
        let results = anime::get_anime(title).await;
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
