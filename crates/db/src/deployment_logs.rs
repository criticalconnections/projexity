use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub seq: i64,
    pub stream: String,
    pub ts: DateTime<Utc>,
    pub text: String,
}

/// Append lines, assigning sequential seq numbers after the current max.
/// Single-writer per deployment (the worker owning the job), so max+1 is
/// race-free in practice; the PK makes any bug loud.
pub async fn append(
    pool: &PgPool,
    deployment_id: Uuid,
    stream: &str,
    lines: &[String],
) -> anyhow::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    let next: i64 = sqlx::query(
        "SELECT COALESCE(MAX(seq), 0) + 1 AS next FROM deployment_logs WHERE deployment_id = $1",
    )
    .bind(deployment_id)
    .fetch_one(&mut *tx)
    .await?
    .get("next");
    for (i, line) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO deployment_logs (deployment_id, seq, stream, text)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(deployment_id)
        .bind(next + i as i64)
        .bind(stream)
        .bind(line)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fetch lines after `since_seq` (exclusive) — the SSE resume cursor.
pub async fn fetch_since(
    pool: &PgPool,
    deployment_id: Uuid,
    since_seq: i64,
    limit: i64,
) -> anyhow::Result<Vec<LogRow>> {
    let rows = sqlx::query(
        "SELECT seq, stream, ts, text FROM deployment_logs
         WHERE deployment_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
    )
    .bind(deployment_id)
    .bind(since_seq)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| LogRow {
            seq: r.get("seq"),
            stream: r.get("stream"),
            ts: r.get("ts"),
            text: r.get("text"),
        })
        .collect())
}
