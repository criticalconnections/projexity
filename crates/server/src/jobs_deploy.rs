//! The `run_deployment` job: execute a release snapshot against the target,
//! streaming provider events into deployment_logs and driving the deployment
//! state machine.
//!
//! Deploy failures are recorded on the deployment row and the job itself
//! succeeds — the row is the source of truth, and a blind job retry of a
//! failed deploy helps nobody. (Transient transport errors surface the same
//! way; "Deploy again" is one click.)

use projexity_core::{DeployEvent, EventSink};
use projexity_db::{deployment_logs, deployments};
use projexity_provider_docker::docker::DockerServer;
use serde::Deserialize;
use uuid::Uuid;

use crate::release::ReleaseSnapshot;
use crate::routes_targets::channel_for;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct Payload {
    deployment_id: Uuid,
}

pub async fn run(state: &AppState, payload: serde_json::Value) -> anyhow::Result<()> {
    let Payload { deployment_id } = serde_json::from_value(payload)?;
    let deployment = deployments::find(&state.pool, deployment_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("deployment {deployment_id} no longer exists"))?;

    // Resume-friendly guard: pending → deploying; a crashed job re-enters at
    // deploying (provider steps are idempotent).
    if deployment.status == "pending" {
        deployments::transition(&state.pool, deployment_id, &["pending"], "deploying").await?;
    } else if deployment.status != "deploying" {
        tracing::info!(%deployment_id, status = %deployment.status, "skipping job for settled deployment");
        return Ok(());
    }

    let result = execute(state, &deployment).await;

    match result {
        Ok(container) => {
            deployments::set_provider_ref(
                &state.pool,
                deployment_id,
                &serde_json::json!({ "container": container }),
            )
            .await?;
            deployments::transition(&state.pool, deployment_id, &["deploying"], "verifying")
                .await?;
            deployments::transition(&state.pool, deployment_id, &["verifying"], "running").await?;
            deployments::supersede_previous(&state.pool, deployment.project_id, deployment_id)
                .await?;
            deployment_logs::append(
                &state.pool,
                deployment_id,
                "deploy",
                &["✓ release is live".to_string()],
            )
            .await?;
        }
        Err(e) => {
            let msg = format!("✗ deploy failed: {e:#}");
            let _ = deployment_logs::append(&state.pool, deployment_id, "deploy", &[msg]).await;
            let _ = deployments::set_error(&state.pool, deployment_id, &format!("{e:#}")).await;
            let _ = deployments::transition(
                &state.pool,
                deployment_id,
                &["pending", "deploying", "verifying"],
                "failed",
            )
            .await;
        }
    }
    Ok(())
}

async fn execute(state: &AppState, deployment: &deployments::Deployment) -> anyhow::Result<String> {
    let snapshot: ReleaseSnapshot = serde_json::from_value(deployment.release_spec.clone())?;
    let project = projexity_db::projects::find(&state.pool, deployment.project_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("project no longer exists"))?;
    let target_id = project
        .target_id
        .ok_or_else(|| anyhow::anyhow!("project has no target"))?;
    let target = projexity_db::targets::find(&state.pool, target_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("target no longer exists"))?;
    if target.status != "ready" {
        anyhow::bail!(
            "server '{}' isn't ready (status: {})",
            target.name,
            target.status
        );
    }
    let config = target.docker_config()?;

    let spec = snapshot.to_spec(
        projexity_core::ProjectId(deployment.project_id),
        &state.master_key,
    )?;
    let channel = channel_for(state, target.id, &config)?;
    let server = DockerServer { channel };

    // Persist provider events as log lines while the deploy runs.
    let (events, mut rx) = EventSink::new(projexity_core::DeploymentId(deployment.id));
    let pool = state.pool.clone();
    let log_id = deployment.id;
    let writer = tokio::spawn(async move {
        while let Some(env) = rx.recv().await {
            let line = match env.event {
                DeployEvent::StepStarted { step } => format!("→ {step}"),
                DeployEvent::StepCompleted { step } => format!("✓ {step}"),
                DeployEvent::Progress { message } => message,
                DeployEvent::HealthProbe { healthy, detail } => {
                    format!(
                        "health: {} ({detail})",
                        if healthy { "ok" } else { "failing" }
                    )
                }
                DeployEvent::TrafficShifted { .. } => "traffic shifted to new release".to_string(),
                DeployEvent::Warning { message } => format!("⚠ {message}"),
            };
            let _ = deployment_logs::append(&pool, log_id, "deploy", &[line]).await;
        }
    });

    let result = server.deploy_release(&spec, &events).await;
    drop(events);
    let _ = writer.await;
    result.map_err(|e| anyhow::anyhow!(e))
}

#[derive(Debug, Deserialize)]
struct DestroyPayload {
    target_id: Uuid,
    slug: String,
}

/// The `destroy_app` job: tear down a deleted project's containers + routes.
pub async fn destroy(state: &AppState, payload: serde_json::Value) -> anyhow::Result<()> {
    let DestroyPayload { target_id, slug } = serde_json::from_value(payload)?;
    let Some(target) = projexity_db::targets::find(&state.pool, target_id).await? else {
        return Ok(()); // target gone; nothing reachable to clean
    };
    let config = target.docker_config()?;
    let channel = channel_for(state, target.id, &config)?;
    let server = DockerServer { channel };
    server
        .destroy_app(&slug)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}
