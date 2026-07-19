use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct TemplateDeployment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub target_id: Uuid,
    pub template_id: String,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub status_detail: String,
    #[serde(skip)]
    pub env_enc: String,
    pub domains: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

const COLS: &str = "id, user_id, target_id, template_id, name, slug, status, status_detail, \
                    env_enc, domains, created_at";

fn row_to_td(r: sqlx::postgres::PgRow) -> TemplateDeployment {
    TemplateDeployment {
        id: r.get("id"),
        user_id: r.get("user_id"),
        target_id: r.get("target_id"),
        template_id: r.get("template_id"),
        name: r.get("name"),
        slug: r.get("slug"),
        status: r.get("status"),
        status_detail: r.get("status_detail"),
        env_enc: r.get("env_enc"),
        domains: r.get("domains"),
        created_at: r.get("created_at"),
    }
}

/// Returns None when the slug is taken.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    target_id: Uuid,
    template_id: &str,
    name: &str,
    slug: &str,
    env_enc: &str,
    domains: &serde_json::Value,
) -> anyhow::Result<Option<TemplateDeployment>> {
    let row = sqlx::query(&format!(
        "INSERT INTO template_deployments
             (id, user_id, target_id, template_id, name, slug, env_enc, domains)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (slug) DO NOTHING
         RETURNING {COLS}"
    ))
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(target_id)
    .bind(template_id)
    .bind(name)
    .bind(slug)
    .bind(env_enc)
    .bind(domains)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_td))
}

pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> anyhow::Result<Vec<TemplateDeployment>> {
    let rows = sqlx::query(&format!(
        "SELECT {COLS} FROM template_deployments WHERE user_id = $1 ORDER BY created_at DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_td).collect())
}

pub async fn find(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<TemplateDeployment>> {
    let row = sqlx::query(&format!(
        "SELECT {COLS} FROM template_deployments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_td))
}

pub async fn find_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> anyhow::Result<Option<TemplateDeployment>> {
    let row = sqlx::query(&format!(
        "SELECT {COLS} FROM template_deployments WHERE id = $1 AND user_id = $2"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_td))
}

pub async fn set_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    status_detail: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE template_deployments SET status = $2, status_detail = $3, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(status_detail)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_status_only(pool: &PgPool, id: Uuid, status: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE template_deployments SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM template_deployments WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
