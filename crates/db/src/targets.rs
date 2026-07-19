use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Config payload for `kind = docker_server` targets, stored as JSONB.
/// The private key is encrypted with the master key before it gets here;
/// nothing in this struct is plaintext-sensitive except `host`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerServerConfig {
    pub host: String,
    pub port: u16,
    pub ssh_user: String,
    pub private_key_enc: String,
    pub public_key: String,
    /// SSH host key recorded on first successful connection (TOFU). A later
    /// mismatch means the server was reinstalled — surfaced, never silently
    /// accepted.
    #[serde(default)]
    pub host_key: Option<String>,
    /// Last preflight facts, for display.
    #[serde(default)]
    pub facts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub status_detail: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl Target {
    pub fn docker_config(&self) -> anyhow::Result<DockerServerConfig> {
        Ok(serde_json::from_value(self.config.clone())?)
    }
}

fn row_to_target(r: sqlx::postgres::PgRow) -> Target {
    Target {
        id: r.get("id"),
        user_id: r.get("user_id"),
        name: r.get("name"),
        kind: r.get("kind"),
        status: r.get("status"),
        status_detail: r.get("status_detail"),
        config: r.get("config"),
        created_at: r.get("created_at"),
    }
}

const COLS: &str = "id, user_id, name, kind, status, status_detail, config, created_at";

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    kind: &str,
    config: &serde_json::Value,
) -> anyhow::Result<Target> {
    let row = sqlx::query(&format!(
        "INSERT INTO targets (id, user_id, name, kind, config)
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLS}"
    ))
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(name)
    .bind(kind)
    .bind(config)
    .fetch_one(pool)
    .await?;
    Ok(row_to_target(row))
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> anyhow::Result<Vec<Target>> {
    let rows = sqlx::query(&format!(
        "SELECT {COLS} FROM targets WHERE user_id = $1 ORDER BY created_at"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_target).collect())
}

pub async fn find_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> anyhow::Result<Option<Target>> {
    let row = sqlx::query(&format!(
        "SELECT {COLS} FROM targets WHERE id = $1 AND user_id = $2"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_target))
}

/// Fetch by id alone — for the background worker, which acts on job payloads
/// rather than on behalf of a session.
pub async fn find(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Target>> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM targets WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(row_to_target))
}

pub async fn update_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    status_detail: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE targets SET status = $2, status_detail = $3, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(status_detail)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update status only, preserving whatever status_detail already holds
/// (used at the end of bootstrap, where detail was streamed step by step).
pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE targets SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_config(
    pool: &PgPool,
    id: Uuid,
    config: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE targets SET config = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(config)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM targets WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() == 1)
}
