use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Build {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub commit_sha: Option<String>,
    pub commit_message: Option<String>,
    pub image_ref: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

const COLS: &str = "id, project_id, status, commit_sha, commit_message, image_ref, error, \
                    created_at, started_at, finished_at";

fn row_to_build(r: sqlx::postgres::PgRow) -> Build {
    Build {
        id: r.get("id"),
        project_id: r.get("project_id"),
        status: r.get("status"),
        commit_sha: r.get("commit_sha"),
        commit_message: r.get("commit_message"),
        image_ref: r.get("image_ref"),
        error: r.get("error"),
        created_at: r.get("created_at"),
        started_at: r.get("started_at"),
        finished_at: r.get("finished_at"),
    }
}

pub async fn create(pool: &PgPool, project_id: Uuid) -> anyhow::Result<Build> {
    let row = sqlx::query(&format!(
        "INSERT INTO builds (id, project_id, status, started_at)
         VALUES ($1, $2, 'cloning', now()) RETURNING {COLS}"
    ))
    .bind(Uuid::now_v7())
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(row_to_build(row))
}

pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE builds SET status = $2,
             finished_at = CASE WHEN $2 IN ('succeeded','failed','canceled','superseded')
                                THEN now() ELSE finished_at END
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_commit(pool: &PgPool, id: Uuid, sha: &str, message: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE builds SET commit_sha = $2, commit_message = $3 WHERE id = $1")
        .bind(id)
        .bind(sha)
        .bind(message)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_image(pool: &PgPool, id: Uuid, image_ref: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE builds SET image_ref = $2 WHERE id = $1")
        .bind(id)
        .bind(image_ref)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_error(pool: &PgPool, id: Uuid, error: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE builds SET error = $2 WHERE id = $1")
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    Ok(())
}
