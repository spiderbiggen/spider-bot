use serenity::builder::CreateApplicationCommands;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::autocomplete::AutocompleteInteraction;
use serenity::prelude::Context;
use serenity::utils::Color;
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

pub(crate) fn register_commands(
    commands: &mut CreateApplicationCommands,
) -> &mut CreateApplicationCommands {
    gifs::register_commands(commands);

    commands
}

pub(crate) async fn interaction(
    command: ApplicationCommandInteraction,
    context: &Context,
    bot: &SpiderBot,
) {
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

pub(crate) async fn autocomplete(command: AutocompleteInteraction, context: &Context) {
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
    interaction: &ApplicationCommandInteraction,
    command_name: &str,
) -> Result<(), CommandError> {
    error!("Received unknown command `{command_name}`, check command mappings");
    let message = format!("{command_name} does not exist!");
    send_error_interaction(ctx, interaction, "Unknown command", &message).await
}

async fn handle_unknown_autocomplete(
    ctx: &Context,
    interaction: &AutocompleteInteraction,
    command_name: &str,
) -> Result<(), CommandError> {
    error!("Received autocomplete request for unknown command `{command_name}`, check command mappings");
    interaction
        .create_autocomplete_response(ctx, |response| response)
        .await?;
    Ok(())
}

async fn send_error_interaction(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    title: &str,
    message: &str,
) -> Result<(), CommandError> {
    interaction
        .delete_original_interaction_response(ctx)
        .await?;
    interaction
        .create_interaction_response(ctx, |response| {
            response.interaction_response_data(|data| {
                data.ephemeral(true).embed(|embed| {
                    embed
                        .color(Color::DARK_RED)
                        .title(title)
                        .description(message)
                })
            })
        })
        .await?;
    Ok(())
}
