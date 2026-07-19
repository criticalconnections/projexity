//! Postgres-backed job queue.
//!
//! Claims use `FOR UPDATE SKIP LOCKED` so any number of worker tasks can pull
//! concurrently; a lease heartbeat makes crashed workers' jobs reclaimable.
//! Long jobs (builds are minutes) must call [`heartbeat`] periodically.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// How long a claim lasts without a heartbeat before the job is considered
/// abandoned and reclaimed.
pub const LEASE_SECONDS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub max_attempts: i32,
}

/// Enqueue a job to run at (or after) `run_at`; `None` means now.
pub async fn enqueue(
    pool: &PgPool,
    kind: &str,
    payload: serde_json::Value,
    run_at: Option<DateTime<Utc>>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, run_at) VALUES ($1, $2, $3, COALESCE($4, now()))",
    )
    .bind(id)
    .bind(kind)
    .bind(payload)
    .bind(run_at)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Claim the next runnable job, if any. A job is runnable when it's queued
/// and due, OR running with an expired lease (crashed worker).
pub async fn claim(pool: &PgPool) -> anyhow::Result<Option<Job>> {
    let row = sqlx::query(
        r#"
        UPDATE jobs SET
            status = 'running',
            attempts = attempts + 1,
            lease_expires_at = now() + make_interval(secs => $1),
            updated_at = now()
        WHERE id = (
            SELECT id FROM jobs
            WHERE (status = 'queued' AND run_at <= now())
               OR (status = 'running' AND lease_expires_at < now())
            ORDER BY run_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING id, kind, payload, attempts, max_attempts
        "#,
    )
    .bind(LEASE_SECONDS as f64)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Job {
        id: r.get("id"),
        kind: r.get("kind"),
        payload: r.get("payload"),
        attempts: r.get("attempts"),
        max_attempts: r.get("max_attempts"),
    }))
}

/// Extend the lease on a running job. Returns false if the job is no longer
/// ours (lease already reclaimed) — the worker must abandon it.
pub async fn heartbeat(pool: &PgPool, job_id: Uuid) -> anyhow::Result<bool> {
    let res = sqlx::query(
        r#"
        UPDATE jobs SET
            lease_expires_at = now() + make_interval(secs => $1),
            updated_at = now()
        WHERE id = $2 AND status = 'running'
        "#,
    )
    .bind(LEASE_SECONDS as f64)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

pub async fn succeed(pool: &PgPool, job_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'succeeded', lease_expires_at = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Record a failure. Retries with linear backoff until `max_attempts` is
/// exhausted, then the job is failed permanently.
pub async fn fail(pool: &PgPool, job_id: Uuid, error: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs SET
            status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
            run_at = now() + make_interval(secs => attempts * 30),
            lease_expires_at = NULL,
            last_error = $2,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Test helper: age a running job's lease so reclaim paths can be exercised
/// without waiting out the real lease.
pub async fn expire_lease_for_test(pool: &PgPool, job_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE jobs SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(job_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Prune terminal jobs older than `keep`.
pub async fn prune(pool: &PgPool, keep: Duration) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM jobs
         WHERE status IN ('succeeded', 'failed')
           AND updated_at < now() - make_interval(secs => $1)",
    )
    .bind(keep.num_seconds() as f64)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
