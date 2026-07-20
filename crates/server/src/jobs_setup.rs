//! The `setup_server` job: run the idempotent bootstrap against a target,
//! streaming per-step progress into `targets.status_detail` (which the wizard
//! polls).

use projexity_provider_docker::bootstrap::Bootstrap;
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::routes_targets::channel_for;
use crate::sshfiles;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct Payload {
    target_id: Uuid,
}

pub async fn run(state: &AppState, payload: serde_json::Value) -> anyhow::Result<()> {
    let Payload { target_id } = serde_json::from_value(payload)?;
    let target = projexity_db::targets::find(&state.pool, target_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("target {target_id} no longer exists"))?;
    let mut config = target.docker_config()?;

    let channel = channel_for(state, target.id, &config)?;

    // Stream step reports into status_detail as they happen.
    let (tx, mut rx) = mpsc::unbounded_channel();
    let pool = state.pool.clone();
    let progress_writer = tokio::spawn(async move {
        while let Some(steps) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&steps) {
                let _ =
                    projexity_db::targets::update_status(&pool, target_id, "bootstrapping", &json)
                        .await;
            }
        }
    });

    let result = Bootstrap::new(&channel, target.id.to_string(), tx)
        .run()
        .await;
    // Sender dropped by run(); wait for the last status write to land so the
    // final set_status below doesn't race it.
    let _ = progress_writer.await;

    match result {
        Ok(facts) => {
            if let Some(kh) = sshfiles::read_known_hosts(&state.config.state_dir, target.id) {
                config.host_key = Some(kh);
            }
            config.facts = Some(serde_json::to_value(&facts)?);
            projexity_db::targets::update_config(
                &state.pool,
                target.id,
                &serde_json::to_value(&config)?,
            )
            .await?;
            projexity_db::targets::set_status(&state.pool, target.id, "ready").await?;
            // Re-render proxy routes so Repair also reconciles routing/TLS
            // policy changes (best effort — a fresh server has none yet).
            let channel = channel_for(state, target.id, &config)?;
            let server = projexity_provider_docker::docker::DockerServer { channel };
            match server.docker().await {
                Ok((docker, _guard)) => {
                    if let Err(e) = server.sync_caddy(&docker, None).await {
                        tracing::warn!(?e, "post-bootstrap proxy sync failed");
                    }
                }
                Err(e) => tracing::warn!(?e, "post-bootstrap proxy sync skipped"),
            }
            tracing::info!(%target_id, "server bootstrap complete");
            Ok(())
        }
        Err(e) => {
            projexity_db::targets::set_status(&state.pool, target.id, "error").await?;
            tracing::warn!(%target_id, error = %e, "server bootstrap failed");
            // The step detail already carries the user-facing message; the
            // job error is for the jobs table.
            Err(e)
        }
    }
}
