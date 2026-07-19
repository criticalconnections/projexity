//! Minimal opaque-token session store. The token travels in an HttpOnly
//! cookie; only its existence and expiry live here.

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub const SESSION_TTL_DAYS: i64 = 30;

/// Create a session and return its opaque token (244 bits of entropy).
pub async fn create(pool: &PgPool, user_id: Uuid) -> anyhow::Result<String> {
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let expires_at: DateTime<Utc> = Utc::now() + Duration::days(SESSION_TTL_DAYS);
    sqlx::query("INSERT INTO sessions (token, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(token)
}

/// Resolve a token to a user id, if the session is live.
pub async fn resolve(pool: &PgPool, token: &str) -> anyhow::Result<Option<Uuid>> {
    let row = sqlx::query("SELECT user_id FROM sessions WHERE token = $1 AND expires_at > now()")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get("user_id")))
}

pub async fn delete(pool: &PgPool, token: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Sweep expired sessions (called periodically by the worker).
pub async fn prune_expired(pool: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
