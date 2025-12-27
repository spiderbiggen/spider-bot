use domain::{Subscriber, UserBalance};
use futures_util::TryStreamExt;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlx_macros::migrate;
use sqlx::{Executor, Postgres};
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

#[derive(thiserror::Error, Debug)]
pub enum BalanceTransactionError {
    #[error(transparent)]
    Base(#[from] Error),
    #[error("insufficient balance: {0}")]
    InsufficientBalance(i64),
    #[error("sender did not exist")]
    SenderUninitialized,
    #[error("recipient did not exist")]
    RecipientUninitialized,
}

impl From<sqlx::Error> for BalanceTransactionError {
    fn from(err: sqlx::Error) -> Self {
        Self::Base(err.into())
    }
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
    /// Returns an error when the database cannot be reached or contains invalid data
    fn get_subscribers(
        &self,
        title: &str,
    ) -> impl Future<Output = Result<Option<Vec<Subscriber>>, Self::Error>>;
}

pub trait UserBalanceConnection {
    type Error: std::error::Error;

    fn create_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        initial_value: i64,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    fn get_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
    ) -> impl Future<Output = Result<Option<i64>, Self::Error>>;

    fn set_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        amount: i64,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    fn get_top_user_balances(
        &self,
        guild_id: u64,
    ) -> impl Future<Output = Result<Vec<UserBalance>, Self::Error>>;

    fn add_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        value: i64,
    ) -> impl Future<Output = Result<i64, Self::Error>>;

    fn upsert_update_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        delta: i64,
        initial_balance: i64,
    ) -> impl Future<Output = Result<i64, Self::Error>>;

    fn upsert_set_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        balance: i64,
    ) -> impl Future<Output = Result<i64, Self::Error>>;
}

pub trait UserBalanceTransaction {
    type Error: std::error::Error;
    fn transfer_user_balance(
        &self,
        guild_id: u64,
        from: u64,
        to: u64,
        value: i64,
    ) -> impl Future<Output = Result<(i64, i64), Self::Error>>;
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

#[derive(Debug, Clone)]
pub struct BotDatabase(PgPool);

impl Deref for BotDatabase {
    type Target = PgPool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
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

impl UserBalanceConnection for BotDatabase {
    type Error = Error;

    async fn create_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        initial_value: i64,
    ) -> Result<(), Self::Error> {
        create_user_balance(&**self, guild_id, user_id, initial_value).await
    }

    async fn get_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
    ) -> Result<Option<i64>, Self::Error> {
        get_user_balance(&**self, guild_id, user_id).await
    }

    async fn set_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        amount: i64,
    ) -> Result<(), Self::Error> {
        set_user_balance(&**self, guild_id, user_id, amount).await
    }

    async fn get_top_user_balances(&self, guild_id: u64) -> Result<Vec<UserBalance>, Self::Error> {
        #[expect(clippy::cast_possible_wrap)]
        let value = sqlx::query_file!("queries/balance/get_top_user_balances.sql", guild_id as i64)
            .fetch(&**self)
            .map_ok(|record| UserBalance {
                #[expect(clippy::cast_sign_loss)]
                user_id: record.user_id as u64,
                balance: record.balance,
            })
            .try_collect()
            .await?;

        Ok(value)
    }

    async fn add_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        value: i64,
    ) -> Result<i64, Self::Error> {
        add_user_balance(&**self, guild_id, user_id, value).await
    }

    async fn upsert_update_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        delta: i64,
        initial_balance: i64,
    ) -> Result<i64, Self::Error> {
        upsert_update_user_balance(&**self, guild_id, user_id, delta, initial_balance).await
    }

    async fn upsert_set_user_balance(
        &self,
        guild_id: u64,
        user_id: u64,
        balance: i64,
    ) -> Result<i64, Self::Error> {
        upsert_set_user_balance(&**self, guild_id, user_id, balance).await
    }
}

impl UserBalanceTransaction for BotDatabase {
    type Error = BalanceTransactionError;

    async fn transfer_user_balance(
        &self,
        guild_id: u64,
        from: u64,
        to: u64,
        value: i64,
    ) -> Result<(i64, i64), Self::Error> {
        let mut transaction = self.begin().await?;
        let Some(from_balance) = get_user_balance(&mut *transaction, guild_id, from).await? else {
            return Err(BalanceTransactionError::SenderUninitialized);
        };
        if from_balance < value {
            return Err(BalanceTransactionError::InsufficientBalance(from_balance));
        }
        get_user_balance(&mut *transaction, guild_id, to)
            .await?
            .ok_or(BalanceTransactionError::RecipientUninitialized)?;
        let new_from_balance = add_user_balance(&**self, guild_id, from, -value).await?;
        let new_to_balance = add_user_balance(&**self, guild_id, to, value).await?;
        transaction.commit().await?;
        Ok((new_from_balance, new_to_balance))
    }
}

async fn create_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
    initial_value: i64,
) -> Result<(), Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    sqlx::query_file!(
        "queries/balance/create_user_balance.sql",
        guild_id as i64,
        user_id as i64,
        initial_value,
    )
    .execute(executor)
    .await?;
    Ok(())
}

async fn get_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
) -> Result<Option<i64>, Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    let value = sqlx::query_file!(
        "queries/balance/get_user_balance.sql",
        guild_id as i64,
        user_id as i64,
    )
    .fetch_optional(executor)
    .await?
    .map(|record| record.balance);
    Ok(value)
}

async fn set_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
    amount: i64,
) -> Result<(), Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    sqlx::query_file!(
        "queries/balance/set_user_balance.sql",
        guild_id as i64,
        user_id as i64,
        amount,
    )
    .execute(executor)
    .await?;
    Ok(())
}

async fn add_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
    value: i64,
) -> Result<i64, Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    let value = sqlx::query_file_scalar!(
        "queries/balance/add_user_balance.sql",
        guild_id as i64,
        user_id as i64,
        value,
    )
    .fetch_one(executor)
    .await?;

    Ok(value)
}

async fn upsert_update_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
    delta: i64,
    initial_balance: i64,
) -> Result<i64, Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    let balance = sqlx::query_file_scalar!(
        "queries/balance/upsert_update_user_balance.sql",
        guild_id as i64,
        user_id as i64,
        initial_balance,
        delta,
    )
    .fetch_one(executor)
    .await?;

    Ok(balance)
}

async fn upsert_set_user_balance<'e, E>(
    executor: E,
    guild_id: u64,
    user_id: u64,
    balance: i64,
) -> Result<i64, Error>
where
    E: Executor<'e, Database = Postgres>,
{
    #[expect(clippy::cast_possible_wrap)]
    let balance = sqlx::query_file_scalar!(
        "queries/balance/upsert_set_user_balance.sql",
        guild_id as i64,
        user_id as i64,
        balance,
    )
    .fetch_one(executor)
    .await?;

    Ok(balance)
}
