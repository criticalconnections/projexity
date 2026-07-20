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
        Ok(provider_ref_json) => {
            let provider_ref: serde_json::Value = serde_json::from_str(&provider_ref_json)
                .unwrap_or_else(|_| serde_json::json!({ "ref": provider_ref_json }));
            deployments::set_provider_ref(&state.pool, deployment_id, &provider_ref).await?;
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
    let spec = snapshot.to_spec(
        projexity_core::ProjectId(deployment.project_id),
        &state.master_key,
    )?;

    // Persist provider events as log lines while the deploy runs (shared by
    // both provider paths).
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

    let result = match target.kind.as_str() {
        "k8s_cluster" => deploy_k8s(state, &target, &snapshot, &spec, &events).await,
        _ => deploy_docker(state, &target, deployment, &snapshot, &spec, &events).await,
    };
    drop(events);
    let _ = writer.await;
    result
}

/// Docker-server path: optional remote build, then blue/green choreography.
async fn deploy_docker(
    state: &AppState,
    target: &projexity_db::targets::Target,
    deployment: &deployments::Deployment,
    snapshot: &ReleaseSnapshot,
    spec: &projexity_core::ReleaseSpec,
    events: &EventSink,
) -> anyhow::Result<String> {
    let config = target.docker_config()?;
    let channel = channel_for(state, target.id, &config)?;
    let server = DockerServer { channel };

    if let Some(repo) = &snapshot.repo {
        build_phase(state, deployment, repo, snapshot, &server, events).await?;
    }
    server
        .deploy_release(spec, events)
        .await
        .map(|container| serde_json::json!({ "container": container }).to_string())
        .map_err(|e| anyhow::anyhow!(e))
}

/// Kubernetes path: server-side apply the manifests and watch the rollout.
/// Git builds aren't supported here yet (they need an in-platform registry so
/// the cluster can pull the image) — surfaced as a clear error.
async fn deploy_k8s(
    state: &AppState,
    target: &projexity_db::targets::Target,
    snapshot: &ReleaseSnapshot,
    spec: &projexity_core::ReleaseSpec,
    events: &EventSink,
) -> anyhow::Result<String> {
    if snapshot.repo.is_some() {
        anyhow::bail!(
            "building from a git repo isn't supported on Kubernetes targets yet — deploy a \
             prebuilt image (a registry the cluster can pull from is coming)"
        );
    }
    let config: projexity_provider_k8s::K8sConfig = serde_json::from_value(target.config.clone())?;
    let kubeconfig = String::from_utf8(state.master_key.decrypt(&config.kubeconfig_enc)?)?;
    let provider = projexity_provider_k8s::K8sProvider::new(&kubeconfig, config)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let provider_ref = provider
        .deploy_release(spec, events)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(provider_ref.to_string())
}

/// Clone → detect plan → pack context → remote build. Streams progress into
/// the event sink and records a `builds` row for history.
async fn build_phase(
    state: &AppState,
    deployment: &deployments::Deployment,
    repo: &crate::release::RepoSpec,
    snapshot: &ReleaseSnapshot,
    server: &DockerServer,
    events: &EventSink,
) -> anyhow::Result<()> {
    let build = projexity_db::builds::create(&state.pool, deployment.project_id).await?;
    let result = run_build_steps(state, &build, repo, snapshot, server, events).await;
    match &result {
        Ok(()) => projexity_db::builds::set_status(&state.pool, build.id, "succeeded").await?,
        Err(e) => {
            projexity_db::builds::set_error(&state.pool, build.id, &format!("{e:#}")).await?;
            projexity_db::builds::set_status(&state.pool, build.id, "failed").await?;
        }
    }
    result
}

async fn run_build_steps(
    state: &AppState,
    build: &projexity_db::builds::Build,
    repo: &crate::release::RepoSpec,
    snapshot: &ReleaseSnapshot,
    server: &DockerServer,
    events: &EventSink,
) -> anyhow::Result<()> {
    use projexity_build::{clone, context, plan};

    events.step_started("clone");
    events.progress(format!(
        "Cloning {}/{} ({})",
        repo.owner, repo.name, repo.branch
    ));
    let workdir = tempfile::tempdir()?;
    // Private repos authenticate with a short-lived installation token (when
    // a GitHub App is configured); public repos clone anonymously.
    let auth_header = github_clone_auth(state).await;
    let cloned = clone::shallow_clone(
        &repo.clone_url(),
        &repo.branch,
        workdir.path(),
        auth_header.as_deref(),
    )
    .await?;
    projexity_db::builds::set_commit(&state.pool, build.id, &cloned.sha, &cloned.message).await?;
    events.progress(format!(
        "at {} — {}",
        &cloned.sha[..cloned.sha.len().min(8)],
        cloned.message
    ));
    events.step_completed("clone");

    events.step_started("build");
    projexity_db::builds::set_status(&state.pool, build.id, "building").await?;
    let detected = plan::detect(
        workdir.path(),
        repo.dockerfile_path.as_deref(),
        snapshot.container_port,
    )?;
    events.progress(detected.summary());
    let tarball = context::pack(
        workdir.path(),
        &detected,
        projexity_build::DEFAULT_CONTEXT_LIMIT_BYTES,
    )?;
    events.progress(format!(
        "build context: {:.1} MiB",
        tarball.len() as f64 / (1024.0 * 1024.0)
    ));
    server
        .build_image(tarball, &snapshot.image, events)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    projexity_db::builds::set_image(&state.pool, build.id, &snapshot.image).await?;
    events.progress(format!("built {}", snapshot.image));
    events.step_completed("build");
    Ok(())
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
    if target.kind == "k8s_cluster" {
        let config: projexity_provider_k8s::K8sConfig =
            serde_json::from_value(target.config.clone())?;
        let kubeconfig = String::from_utf8(state.master_key.decrypt(&config.kubeconfig_enc)?)?;
        projexity_provider_k8s::K8sProvider::new(&kubeconfig, config)
            .await
            .map_err(|e| anyhow::anyhow!(e))?
            .destroy_app(&slug)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }
    let config = target.docker_config()?;
    let channel = channel_for(state, target.id, &config)?;
    let server = DockerServer { channel };
    server
        .destroy_app(&slug)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// Best-effort installation-token auth for clones. Returns None (anonymous)
/// when no GitHub App is configured or token minting fails — public repos
/// keep working either way.
async fn github_clone_auth(state: &AppState) -> Option<String> {
    let app = projexity_db::github::get_app(&state.pool).await.ok()??;
    let installations = projexity_db::github::list_installations(&state.pool)
        .await
        .ok()?;
    let inst = installations.first()?;
    let pem = String::from_utf8(state.master_key.decrypt(&app.pem_enc).ok()?).ok()?;
    match projexity_github::app::installation_token(app.app_id, &pem, inst.installation_id).await {
        Ok(token) => Some(projexity_github::app::clone_auth_header(&token)),
        Err(e) => {
            tracing::warn!(?e, "installation token unavailable; cloning anonymously");
            None
        }
    }
}
