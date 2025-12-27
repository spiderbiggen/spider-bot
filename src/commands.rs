use crate::commands::gifs::GifError;
use crate::context::Context;

pub mod gifs;
pub mod true_coin;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommandError {
    #[error(transparent)]
    GifError(#[from] GifError),
    #[error(transparent)]
    Serenity(#[from] serenity::Error),
    #[error(transparent)]
    Database(#[from] db::Error),
}

#[tracing::instrument(skip_all)]
#[poise::command(slash_command)]
pub(crate) async fn version(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    const PKG_REF: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
    ctx.say(PKG_REF).await?;
    Ok(())
}
