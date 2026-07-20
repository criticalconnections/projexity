//! Install/uninstall jobs for one-click apps, streaming step progress into
//! template_deployments.status_detail (same shape the target bootstrap uses).

use std::collections::BTreeMap;

use projexity_core::{DeployEvent, EventSink};
use projexity_db::template_deployments as td;
use projexity_provider_docker::docker::DockerServer;
use serde::Deserialize;
use uuid::Uuid;

use crate::routes_targets::channel_for;
use crate::state::AppState;
use crate::templates;

#[derive(Debug, Deserialize)]
struct Payload {
    template_deployment_id: Uuid,
}

const STEPS: &[(&str, &str)] = &[
    ("upload", "Uploading the stack definition"),
    ("up", "Pulling images and starting services"),
    ("route", "Wiring HTTPS routes"),
];

fn steps_json(current: &str, failed: Option<(&str, &str)>) -> String {
    let states: Vec<serde_json::Value> = STEPS
        .iter()
        .map(|(id, label)| {
            let status = if let Some((fid, _)) = failed {
                if *id == fid {
                    "failed"
                } else if step_index(id) < step_index(fid) {
                    "done"
                } else {
                    "pending"
                }
            } else if step_index(id) < step_index(current) {
                "done"
            } else if *id == current {
                "running"
            } else {
                "pending"
            };
            let detail = match failed {
                Some((fid, msg)) if *id == fid => msg,
                _ => "",
            };
            serde_json::json!({"id": id, "label": label, "status": status, "detail": detail})
        })
        .collect();
    serde_json::to_string(&states).unwrap_or_default()
}

fn step_index(id: &str) -> usize {
    STEPS
        .iter()
        .position(|(s, _)| *s == id)
        .unwrap_or(usize::MAX)
}

pub async fn install(state: &AppState, payload: serde_json::Value) -> anyhow::Result<()> {
    let Payload {
        template_deployment_id: id,
    } = serde_json::from_value(payload)?;
    let d = td::find(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("template deployment {id} no longer exists"))?;
    let template = state
        .templates
        .iter()
        .find(|t| t.id == d.template_id)
        .ok_or_else(|| anyhow::anyhow!("template {} not in catalog", d.template_id))?
        .clone();
    let target = projexity_db::targets::find(&state.pool, d.target_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("target no longer exists"))?;
    let config = target.docker_config()?;

    let env: BTreeMap<String, String> =
        serde_json::from_slice(&state.master_key.decrypt(&d.env_enc)?)?;
    let domains: BTreeMap<String, String> = serde_json::from_value(d.domains.clone())?;
    let compose = templates::render_compose(&template, &d.slug, &env, &domains)?;

    let channel = channel_for(state, target.id, &config)?;
    let server = DockerServer { channel };

    // Progress: translate step events into the steps-JSON the UI polls.
    let (events, mut rx) = EventSink::new(projexity_core::DeploymentId(d.id));
    let pool = state.pool.clone();
    let watcher = tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            if let DeployEvent::StepStarted { step } = &envelope.event {
                let _ = td::set_status(&pool, id, "installing", &steps_json(step, None)).await;
            }
        }
    });

    let result = server.install_stack(&d.slug, &compose, &events).await;
    drop(events);
    let _ = watcher.await;

    match result {
        Ok(()) => {
            td::set_status(&state.pool, id, "running", &steps_json("done", None)).await?;
            tracing::info!(app = %d.slug, "template stack installed");
            Ok(())
        }
        Err(e) => {
            // Which step died is in the error path; mark the likely one.
            let msg = e.to_string();
            let failed_step = if msg.contains("compose up") {
                "up"
            } else if msg.contains("route") || msg.contains("Caddy") {
                "route"
            } else {
                "upload"
            };
            td::set_status(
                &state.pool,
                id,
                "error",
                &steps_json(failed_step, Some((failed_step, &msg))),
            )
            .await?;
            Err(anyhow::anyhow!(e))
        }
    }
}

#[derive(Debug, Deserialize)]
struct UninstallPayload {
    template_deployment_id: Uuid,
    /// Also delete named data volumes (default keeps them).
    #[serde(default)]
    purge: bool,
}

pub async fn uninstall(state: &AppState, payload: serde_json::Value) -> anyhow::Result<()> {
    let UninstallPayload {
        template_deployment_id: id,
        purge,
    } = serde_json::from_value(payload)?;
    let Some(d) = td::find(&state.pool, id).await? else {
        return Ok(());
    };
    if let Some(target) = projexity_db::targets::find(&state.pool, d.target_id).await? {
        let config = target.docker_config()?;
        let channel = channel_for(state, target.id, &config)?;
        let server = DockerServer { channel };
        // `purge` removes named volumes too — deleting all of the app's data.
        server
            .uninstall_stack(&d.slug, purge)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    td::delete(&state.pool, id).await?;
    tracing::info!(app = %d.slug, purge, "template stack removed");
    Ok(())
}
