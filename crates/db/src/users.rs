use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

fn row_to_user(r: sqlx::postgres::PgRow) -> User {
    User {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        created_at: r.get("created_at"),
    }
}

/// Insert a user; returns None if the email is already taken.
pub async fn create(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
) -> anyhow::Result<Option<User>> {
    let row = sqlx::query(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO NOTHING
         RETURNING id, email, password_hash, created_at",
    )
    .bind(Uuid::now_v7())
    .bind(email)
    .bind(password_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_user))
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> anyhow::Result<Option<User>> {
    let row =
        sqlx::query("SELECT id, email, password_hash, created_at FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(row_to_user))
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<User>> {
    let row = sqlx::query("SELECT id, email, password_hash, created_at FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(row_to_user))
}

/// Count of registered users (used to badge the first-run experience).
pub async fn count(pool: &PgPool) -> anyhow::Result<i64> {
    let row = sqlx::query("SELECT count(*) AS n FROM users")
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>("n"))
}
