use serenity::all::{CommandInteraction, CreateMessage, EditInteractionResponse};
use serenity::prelude::Context;

use crate::commands::CommandError;

pub(crate) async fn send_reply(
    ctx: &Context,
    interaction: &CommandInteraction,
    messages: impl IntoIterator<Item = String>,
) -> Result<(), CommandError> {
    let mut iter = messages.into_iter();
    if let Some(msg) = iter.next() {
        interaction
            .edit_response(ctx, EditInteractionResponse::new().content(msg))
            .await?;
    }
    for msg in iter {
        interaction
            .channel_id
            .send_message(ctx, CreateMessage::new().content(msg))
            .await?;
    }

    Ok(())
}
