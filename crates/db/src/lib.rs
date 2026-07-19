//! Postgres persistence for Projexity.
//!
//! Postgres is deliberately the ONLY stateful dependency of the whole
//! platform (no Redis, no external queue) — it keeps self-hosting to one
//! `docker compose up`. The job queue lives here too ([`jobs`]).
//!
//! Queries use sqlx's runtime API (not the compile-time-checked macros) so
//! contributors can build without a database running; integration tests cover
//! the SQL.

pub mod deployment_logs;
pub mod deployments;
pub mod env_vars;
pub mod jobs;
pub mod projects;
pub mod sessions;
pub mod targets;
pub mod users;

use sqlx::postgres::{PgPool, PgPoolOptions};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Connect and run pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}
