use tracing::error;

use crate::commands::gifs::GifError;

pub mod gifs;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommandError {
    #[error(transparent)]
    GifError(#[from] GifError),
    #[error(transparent)]
    Serenity(#[from] serenity::Error),
}
