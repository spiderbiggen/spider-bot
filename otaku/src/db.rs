use std::ops::Deref;

use sqlx::migrate::{Migrate, MigrateError};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlx_macros::migrate;
use sqlx::{Acquire, Postgres};

pub type Pool = sqlx::Pool<Postgres>;

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
pub async fn connect(name: &str) -> Result<sqlx::Pool<Postgres>, sqlx::Error> {
    let (connect_opts, pool_opts) = opts(name);
    let pool = pool_opts.connect_with(connect_opts).await?;
    Ok(pool)
}

/// Migrate the database located in the migrations directory.
///
/// # Errors
///
/// Return an error when the database cannot be reached or when a migration fails.
pub async fn migrate<'a, A>(migrator: A) -> Result<(), MigrateError>
where
    A: Acquire<'a>,
    <A::Connection as Deref>::Target: Migrate,
{
    migrate!("./migrations").run(migrator).await
}
