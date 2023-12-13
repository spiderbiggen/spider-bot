use serenity::all::{
    Color, CommandInteraction, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::all::{CreateAutocompleteResponse, CreateCommand};
use serenity::prelude::Context;
use tracing::error;

use crate::commands::gifs::GifError;
use crate::SpiderBot;

pub mod gifs;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommandError {
    #[error(transparent)]
    GifError(#[from] GifError),
    #[error(transparent)]
    Serenity(#[from] serenity::Error),
}

pub(crate) fn register_commands(commands: &mut Vec<CreateCommand>) {
    gifs::register_commands(commands);
}

pub(crate) async fn interaction(command: CommandInteraction, context: &Context, bot: &SpiderBot) {
    let command_name = command.data.name.as_str();
    let result = match command_name {
        "play" => gifs::play(context, &command, bot).await,
        "hurry" => gifs::hurry(context, &command, bot).await,
        "sleep" => gifs::sleep(context, &command, bot).await,
        cmd => handle_unknown_command(context, &command, cmd).await,
    };

    if let Err(err) = result {
        error!("Error handling command {command_name}. {err}");
        let _ = send_error_interaction(
            context,
            &command,
            "Something went wrong!",
            "Try again later",
        )
        .await;
    }
}

pub(crate) async fn autocomplete(command: CommandInteraction, context: &Context) {
    let command_name = command.data.name.as_str();
    let result = match command_name {
        "play" => gifs::play_autocomplete(context, &command).await,
        cmd => handle_unknown_autocomplete(context, &command, cmd).await,
    };

    if let Err(err) = result {
        error!(err = ?err, "Error handling command {command_name}");
    }
}

async fn handle_unknown_command(
    ctx: &Context,
    interaction: &CommandInteraction,
    command_name: &str,
) -> Result<(), CommandError> {
    error!("Received unknown command `{command_name}`, check command mappings");
    let message = format!("{command_name} does not exist!");
    send_error_interaction(ctx, interaction, "Unknown command", &message).await
}

async fn handle_unknown_autocomplete(
    ctx: &Context,
    interaction: &CommandInteraction,
    command_name: &str,
) -> Result<(), CommandError> {
    error!("Received autocomplete request for unknown command `{command_name}`, check command mappings");
    let response = CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new());
    interaction.create_response(ctx, response).await?;
    Ok(())
}

async fn send_error_interaction(
    ctx: &Context,
    interaction: &CommandInteraction,
    title: &str,
    message: &str,
) -> Result<(), CommandError> {
    interaction.delete_response(ctx).await?;
    let response_embed = CreateEmbed::new()
        .color(Color::DARK_RED)
        .title(title)
        .description(message);
    let response_message = CreateInteractionResponseMessage::new()
        .ephemeral(true)
        .embed(response_embed);
    let response = CreateInteractionResponse::Message(response_message);
    interaction.create_response(ctx, response).await?;
    Ok(())
}
