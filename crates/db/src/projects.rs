use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,
    pub target_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub image: Option<String>,
    pub repo_owner: Option<String>,
    pub repo_name: Option<String>,
    pub branch: String,
    pub container_port: i32,
    pub created_at: DateTime<Utc>,
}

const COLS: &str = "id, user_id, target_id, name, slug, image, repo_owner, repo_name, branch, \
                    container_port, created_at";

fn row_to_project(r: sqlx::postgres::PgRow) -> Project {
    Project {
        id: r.get("id"),
        user_id: r.get("user_id"),
        target_id: r.get("target_id"),
        name: r.get("name"),
        slug: r.get("slug"),
        image: r.get("image"),
        repo_owner: r.get("repo_owner"),
        repo_name: r.get("repo_name"),
        branch: r.get("branch"),
        container_port: r.get("container_port"),
        created_at: r.get("created_at"),
    }
}

/// Insert; returns None when the slug is taken.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    target_id: Uuid,
    name: &str,
    slug: &str,
    image: Option<&str>,
    container_port: i32,
) -> anyhow::Result<Option<Project>> {
    let row = sqlx::query(&format!(
        "INSERT INTO projects (id, user_id, target_id, name, slug, image, container_port)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (slug) DO NOTHING
         RETURNING {COLS}"
    ))
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(target_id)
    .bind(name)
    .bind(slug)
    .bind(image)
    .bind(container_port)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_project))
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> anyhow::Result<Vec<Project>> {
    let rows = sqlx::query(&format!(
        "SELECT {COLS} FROM projects WHERE user_id = $1 ORDER BY created_at"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_project).collect())
}

pub async fn find_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> anyhow::Result<Option<Project>> {
    let row = sqlx::query(&format!(
        "SELECT {COLS} FROM projects WHERE id = $1 AND user_id = $2"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_project))
}

pub async fn find(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Project>> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM projects WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(row_to_project))
}

pub async fn update_image(pool: &PgPool, id: Uuid, image: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE projects SET image = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(image)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM projects WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() == 1)
}

/// Generated domains + env live in their own tables.
pub async fn domains(pool: &PgPool, project_id: Uuid) -> anyhow::Result<Vec<String>> {
    let rows =
        sqlx::query("SELECT hostname FROM domains WHERE project_id = $1 ORDER BY created_at")
            .bind(project_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.get("hostname")).collect())
}

pub async fn add_domain(
    pool: &PgPool,
    project_id: Uuid,
    hostname: &str,
    is_generated: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO domains (id, project_id, hostname, is_generated) VALUES ($1, $2, $3, $4)
         ON CONFLICT (hostname) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(hostname)
    .bind(is_generated)
    .execute(pool)
    .await?;
    Ok(())
}
