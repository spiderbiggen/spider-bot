use domain::Subscriber;
use futures_util::TryStreamExt;
use sqlx::Postgres;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlx_macros::migrate;
use std::num::ParseIntError;
use std::ops::Deref;
use tracing::instrument;

type PgPool = sqlx::Pool<Postgres>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("{0} for {1}")]
    ParseInt(#[source] ParseIntError, &'static str),
}

pub trait DatabaseConnection {
    type Error: std::error::Error;
    type MigrateError: std::error::Error;

    /// Migrate the database to the latest version.
    ///
    /// # Errors
    ///
    /// Return an error when the database cannot be reached or when a migration fails.
    fn migrate(&self) -> impl Future<Output = Result<(), Self::MigrateError>>;

    /// Returns a list of subscribed discord users/channels
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be reached,
    /// contains invalid data or returns no results.
    fn get_subscribers(
        &self,
        title: &str,
    ) -> impl Future<Output = Result<Option<Vec<Subscriber>>, Self::Error>>;
}

#[derive(Debug, Clone)]
pub struct BotDatabase(PgPool);

impl Deref for BotDatabase {
    type Target = PgPool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn opts(name: &str) -> (PgConnectOptions, PgPoolOptions) {
    let connect_opts = PgConnectOptions::new().application_name(name);
    let pool_opts = PgPoolOptions::new().max_connections(2);
    (connect_opts, pool_opts)
}

/// Connect to the database using connection parameters from the environment.
///
/// # Errors
///
/// Will return an error when a connection cannot be established using the current config.
pub async fn connect(name: &str) -> Result<BotDatabase, sqlx::Error> {
    let (connect_opts, pool_opts) = opts(name);
    let pool = pool_opts.connect_with(connect_opts).await?;
    Ok(BotDatabase(pool))
}

impl DatabaseConnection for BotDatabase {
    type Error = Error;
    type MigrateError = sqlx::migrate::MigrateError;

    async fn migrate(&self) -> Result<(), Self::MigrateError> {
        migrate!("./migrations").run(&**self).await
    }

    #[instrument(skip(self))]
    async fn get_subscribers(&self, title: &str) -> Result<Option<Vec<Subscriber>>, Self::Error> {
        let channels: Vec<_> = sqlx::query_file!("queries/find_subscribed_channels.sql", title)
            .fetch(&**self)
            .err_into::<Error>()
            .and_then(|record| async move {
                Ok(Subscriber::Channel {
                    channel_id: record
                        .channel_id
                        .parse()
                        .map_err(|err| Error::ParseInt(err, "channel_id"))?,
                    guild_id: record
                        .guild_id
                        .parse()
                        .map_err(|err| Error::ParseInt(err, "guild_id"))?,
                })
            })
            .try_collect()
            .await?;

        if channels.is_empty() {
            return Ok(None);
        }
        Ok(Some(channels))
    }
}
