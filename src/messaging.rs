use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::prelude::Context;

use crate::commands::CommandError;

pub(crate) async fn send_reply(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    messages: impl IntoIterator<Item = String>,
) -> Result<(), CommandError> {
    let mut iter = messages.into_iter();
    if let Some(msg) = iter.next() {
        interaction
            .edit_original_interaction_response(ctx, |response| response.content(msg))
            .await?;
    }
    for msg in iter {
        interaction
            .channel_id
            .send_message(ctx, |message| message.content(msg))
            .await?;
    }

    Ok(())
}
