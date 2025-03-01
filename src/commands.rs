use tracing::error;

use crate::commands::gifs::GifError;

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
    #[error(transparent)]
    BalanceTransaction(#[from] db::BalanceTransactionError),
}
