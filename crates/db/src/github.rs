use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// This instance's registered GitHub App (credentials encrypted upstream).
#[derive(Debug, Clone, Serialize)]
pub struct GithubApp {
    pub id: Uuid,
    pub user_id: Uuid,
    pub app_id: i64,
    pub slug: String,
    pub html_url: String,
    pub client_id: String,
    #[serde(skip)]
    pub pem_enc: String,
    #[serde(skip)]
    pub webhook_secret_enc: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Installation {
    pub installation_id: i64,
    pub account_login: String,
}

fn row_to_app(r: sqlx::postgres::PgRow) -> GithubApp {
    GithubApp {
        id: r.get("id"),
        user_id: r.get("user_id"),
        app_id: r.get("app_id"),
        slug: r.get("slug"),
        html_url: r.get("html_url"),
        client_id: r.get("client_id"),
        pem_enc: r.get("pem_enc"),
        webhook_secret_enc: r.get("webhook_secret_enc"),
        created_at: r.get("created_at"),
    }
}

/// Store the app registered via the manifest flow. An instance has one app —
/// re-registering replaces it.
#[allow(clippy::too_many_arguments)]
pub async fn save_app(
    pool: &PgPool,
    user_id: Uuid,
    app_id: i64,
    slug: &str,
    html_url: &str,
    client_id: &str,
    pem_enc: &str,
    webhook_secret_enc: &str,
) -> anyhow::Result<GithubApp> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM github_apps")
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "INSERT INTO github_apps
             (id, user_id, app_id, slug, html_url, client_id, pem_enc, webhook_secret_enc)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, user_id, app_id, slug, html_url, client_id, pem_enc,
                   webhook_secret_enc, created_at",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(app_id)
    .bind(slug)
    .bind(html_url)
    .bind(client_id)
    .bind(pem_enc)
    .bind(webhook_secret_enc)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row_to_app(row))
}

pub async fn get_app(pool: &PgPool) -> anyhow::Result<Option<GithubApp>> {
    let row = sqlx::query(
        "SELECT id, user_id, app_id, slug, html_url, client_id, pem_enc,
                webhook_secret_enc, created_at
         FROM github_apps ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_app))
}

pub async fn upsert_installation(
    pool: &PgPool,
    installation_id: i64,
    account_login: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO github_installations (id, installation_id, account_login)
         VALUES ($1, $2, $3)
         ON CONFLICT (installation_id)
         DO UPDATE SET account_login = EXCLUDED.account_login",
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(account_login)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_installation(pool: &PgPool, installation_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM github_installations WHERE installation_id = $1")
        .bind(installation_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_installations(pool: &PgPool) -> anyhow::Result<Vec<Installation>> {
    let rows = sqlx::query(
        "SELECT installation_id, account_login FROM github_installations ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Installation {
            installation_id: r.get("installation_id"),
            account_login: r.get("account_login"),
        })
        .collect())
}
