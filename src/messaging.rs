use serenity::all::{
    CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
};
use serenity::prelude::Context;

use crate::commands::CommandError;

pub(crate) async fn send_reply(
    ctx: &Context,
    interaction: &CommandInteraction,
    messages: impl IntoIterator<Item = String>,
) -> Result<(), CommandError> {
    let mut iter = messages.into_iter();
    if let Some(msg) = iter.next() {
        let interaction_response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(msg),
        );
        interaction
            .create_response(ctx, interaction_response)
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
