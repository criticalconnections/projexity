use sqlx::{PgPool, Row};
use uuid::Uuid;

/// One stored env var. Values arrive already encrypted (the server crate owns
/// the master key); this module never sees plaintext.
#[derive(Debug, Clone)]
pub struct EnvVarRow {
    pub key: String,
    pub value_ciphertext: Vec<u8>,
    pub is_build_time: bool,
}

pub async fn list(pool: &PgPool, project_id: Uuid) -> anyhow::Result<Vec<EnvVarRow>> {
    let rows = sqlx::query(
        "SELECT key, value_ciphertext, is_build_time FROM env_vars
         WHERE project_id = $1 ORDER BY key",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| EnvVarRow {
            key: r.get("key"),
            value_ciphertext: r.get("value_ciphertext"),
            is_build_time: r.get("is_build_time"),
        })
        .collect())
}

/// Replace the whole set atomically (the UI edits env as a document).
pub async fn replace_all(
    pool: &PgPool,
    project_id: Uuid,
    vars: &[EnvVarRow],
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM env_vars WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    for v in vars {
        sqlx::query(
            "INSERT INTO env_vars (id, project_id, key, value_ciphertext, is_build_time)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(&v.key)
        .bind(&v.value_ciphertext)
        .bind(v.is_build_time)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
