use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Deployment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub status: String,
    /// Immutable release snapshot (image, port, domains, encrypted env,
    /// release id). Replaying it reproduces the release — that's rollback.
    pub release_spec: serde_json::Value,
    pub provider_ref: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

const COLS: &str =
    "id, project_id, kind, status, release_spec, provider_ref, error, created_at, finished_at";

fn row_to_deployment(r: sqlx::postgres::PgRow) -> Deployment {
    Deployment {
        id: r.get("id"),
        project_id: r.get("project_id"),
        kind: r.get("kind"),
        status: r.get("status"),
        release_spec: r.get("release_spec"),
        provider_ref: r.get("provider_ref"),
        error: r.get("error"),
        created_at: r.get("created_at"),
        finished_at: r.get("finished_at"),
    }
}

/// Create a pending deployment. Fails with `Ok(None)` when another deployment
/// for this project is already in flight (enforced by the partial unique
/// index — double-enqueue is structurally impossible).
pub async fn create(
    pool: &PgPool,
    project_id: Uuid,
    kind: &str,
    release_spec: &serde_json::Value,
) -> anyhow::Result<Option<Deployment>> {
    let res = sqlx::query(&format!(
        "INSERT INTO deployments (id, project_id, kind, release_spec)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT DO NOTHING
         RETURNING {COLS}"
    ))
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(kind)
    .bind(release_spec)
    .fetch_optional(pool)
    .await;
    match res {
        Ok(row) => Ok(row.map(row_to_deployment)),
        // ON CONFLICT DO NOTHING doesn't apply to partial unique indexes on
        // all PG configs; treat unique violations as "already in flight".
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn find(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Deployment>> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM deployments WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(row_to_deployment))
}

/// Find a deployment scoped to the requesting user (join through projects).
pub async fn find_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> anyhow::Result<Option<Deployment>> {
    let row = sqlx::query(&format!(
        "SELECT d.{} FROM deployments d
         JOIN projects p ON p.id = d.project_id
         WHERE d.id = $1 AND p.user_id = $2",
        COLS.replace(", ", ", d.")
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_deployment))
}

pub async fn list_for_project(
    pool: &PgPool,
    project_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<Deployment>> {
    let rows = sqlx::query(&format!(
        "SELECT {COLS} FROM deployments WHERE project_id = $1
         ORDER BY created_at DESC LIMIT $2"
    ))
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_deployment).collect())
}

pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<Deployment>> {
    let rows = sqlx::query(&format!(
        "SELECT d.{} FROM deployments d
         JOIN projects p ON p.id = d.project_id
         WHERE p.user_id = $1
         ORDER BY d.created_at DESC LIMIT $2",
        COLS.replace(", ", ", d.")
    ))
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_deployment).collect())
}

/// Guarded transition: only fires if the current status allows it (the state
/// machine in projexity-core is the authority; this enforces it in SQL).
pub async fn transition(pool: &PgPool, id: Uuid, from: &[&str], to: &str) -> anyhow::Result<bool> {
    let terminal = matches!(to, "running" | "superseded" | "stopped" | "failed");
    let res = sqlx::query(
        "UPDATE deployments SET status = $2,
             finished_at = CASE WHEN $3 THEN now() ELSE finished_at END
         WHERE id = $1 AND status = ANY($4)",
    )
    .bind(id)
    .bind(to)
    .bind(terminal)
    .bind(from.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

pub async fn set_error(pool: &PgPool, id: Uuid, error: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE deployments SET error = $2 WHERE id = $1")
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_provider_ref(
    pool: &PgPool,
    id: Uuid,
    provider_ref: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE deployments SET provider_ref = $2 WHERE id = $1")
        .bind(id)
        .bind(provider_ref)
        .execute(pool)
        .await?;
    Ok(())
}

/// Mark the previously-running deployment of this project superseded (called
/// once the new release is serving traffic).
pub async fn supersede_previous(
    pool: &PgPool,
    project_id: Uuid,
    new_deployment_id: Uuid,
) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "UPDATE deployments SET status = 'superseded', finished_at = now()
         WHERE project_id = $1 AND id <> $2 AND status = 'running'",
    )
    .bind(project_id)
    .bind(new_deployment_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
