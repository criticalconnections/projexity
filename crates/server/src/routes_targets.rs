use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use projexity_db::targets::{self, DockerServerConfig, Target};
use projexity_provider_docker::{preflight, transport::SshChannel};
use serde::{Deserialize, Serialize};
use ssh_key::{rand_core::OsRng, Algorithm, LineEnding, PrivateKey};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::sshfiles;
use crate::state::AppState;

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(?e, "internal error");
    err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

/// What the API exposes about a target. Never includes key ciphertext.
#[derive(Debug, Serialize)]
pub struct TargetResponse {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub status: String,
    /// JSON-encoded bootstrap step reports (see provider-docker StepReport).
    pub status_detail: String,
    pub host: String,
    pub port: u16,
    pub ssh_user: String,
    pub public_key: String,
    /// One-liner the user runs on the server to authorize our key.
    pub setup_command: String,
    pub facts: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

fn to_response(t: &Target) -> anyhow::Result<TargetResponse> {
    let c = t.docker_config()?;
    Ok(TargetResponse {
        id: t.id,
        name: t.name.clone(),
        kind: t.kind.clone(),
        status: t.status.clone(),
        status_detail: t.status_detail.clone(),
        host: c.host.clone(),
        port: c.port,
        ssh_user: c.ssh_user.clone(),
        public_key: c.public_key.clone(),
        setup_command: format!(
            "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
            c.public_key.trim()
        ),
        facts: c.facts,
        created_at: t.created_at,
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateTarget {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_user")]
    pub ssh_user: String,
}

fn default_port() -> u16 {
    22
}
fn default_user() -> String {
    "root".into()
}

pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<CreateTarget>,
) -> Result<Json<TargetResponse>, Response> {
    let name = req.name.trim();
    let host = req.host.trim();
    let ssh_user = req.ssh_user.trim();
    if name.is_empty() {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "give the server a name",
        ));
    }
    if host.is_empty() || host.contains(char::is_whitespace) || host.contains('@') {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "enter a hostname or IP address (without user@)",
        ));
    }
    if ssh_user.is_empty() || ssh_user.contains(char::is_whitespace) {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "invalid SSH user"));
    }

    // Fresh keypair per server: revoking one server never affects another.
    let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519)
        .map_err(|e| internal(anyhow::anyhow!(e)))?;
    let private_openssh = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| internal(anyhow::anyhow!(e)))?;
    let public_openssh = key
        .public_key()
        .to_openssh()
        .map_err(|e| internal(anyhow::anyhow!(e)))?;

    let config = DockerServerConfig {
        host: host.to_string(),
        port: req.port,
        ssh_user: ssh_user.to_string(),
        private_key_enc: state
            .master_key
            .encrypt(private_openssh.as_bytes())
            .map_err(internal)?,
        public_key: format!("{} projexity", public_openssh.trim()),
        host_key: None,
        facts: None,
    };

    let target = targets::create(
        &state.pool,
        user.id,
        name,
        "docker_server",
        &serde_json::to_value(&config).map_err(|e| internal(e.into()))?,
    )
    .await
    .map_err(internal)?;

    Ok(Json(to_response(&target).map_err(internal)?))
}

pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<TargetResponse>>, Response> {
    let rows = targets::list_for_user(&state.pool, user.id)
        .await
        .map_err(internal)?;
    let mut out = Vec::with_capacity(rows.len());
    for t in &rows {
        out.push(to_response(t).map_err(internal)?);
    }
    Ok(Json(out))
}

async fn load(state: &AppState, user: &CurrentUser, id: Uuid) -> Result<Target, Response> {
    targets::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "target not found"))
}

pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TargetResponse>, Response> {
    let t = load(&state, &user, id).await?;
    Ok(Json(to_response(&t).map_err(internal)?))
}

pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Response> {
    let deleted = targets::delete(&state.pool, user.id, id)
        .await
        .map_err(internal)?;
    if !deleted {
        return Err(err(StatusCode::NOT_FOUND, "target not found"));
    }
    sshfiles::cleanup(&state.config.state_dir, id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub facts: Option<preflight::ServerFacts>,
    pub issues: Vec<preflight::Issue>,
    pub error: Option<String>,
}

/// Test the SSH connection and gather facts. Safe to call repeatedly; the
/// wizard's "test connection" button hits this.
pub async fn check(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckResponse>, Response> {
    let t = load(&state, &user, id).await?;
    let mut config = t.docker_config().map_err(internal)?;

    let channel = channel_for(&state, t.id, &config).map_err(internal)?;
    match preflight::run(&channel).await {
        Ok(facts) => {
            // Pin the host key learned on this (first) connect and remember
            // the facts for display.
            if let Some(kh) = sshfiles::read_known_hosts(&state.config.state_dir, t.id) {
                config.host_key = Some(kh);
            }
            config.facts = Some(serde_json::to_value(&facts).map_err(|e| internal(e.into()))?);
            targets::update_config(
                &state.pool,
                t.id,
                &serde_json::to_value(&config).map_err(|e| internal(e.into()))?,
            )
            .await
            .map_err(internal)?;

            Ok(Json(CheckResponse {
                ok: true,
                issues: facts.issues(),
                facts: Some(facts),
                error: None,
            }))
        }
        Err(e) => Ok(Json(CheckResponse {
            ok: false,
            facts: None,
            issues: vec![],
            error: Some(e.to_string()),
        })),
    }
}

/// Kick off (or re-run — "Repair") the bootstrap as a background job.
pub async fn bootstrap(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TargetResponse>, Response> {
    let t = load(&state, &user, id).await?;
    if t.status == "bootstrapping" {
        return Err(err(StatusCode::CONFLICT, "setup is already running"));
    }

    let initial_steps: Vec<serde_json::Value> = projexity_provider_docker::bootstrap::STEPS
        .iter()
        .map(|(step_id, label)| {
            serde_json::json!({"id": step_id, "label": label, "status": "pending", "detail": ""})
        })
        .collect();
    targets::update_status(
        &state.pool,
        t.id,
        "bootstrapping",
        &serde_json::to_string(&initial_steps).map_err(|e| internal(e.into()))?,
    )
    .await
    .map_err(internal)?;

    projexity_db::jobs::enqueue(
        &state.pool,
        "setup_server",
        serde_json::json!({ "target_id": t.id }),
        None,
    )
    .await
    .map_err(internal)?;

    let t = load(&state, &user, id).await?;
    Ok(Json(to_response(&t).map_err(internal)?))
}

/// Build an SSH channel for a target, materializing key files.
pub fn channel_for(
    state: &AppState,
    target_id: Uuid,
    config: &DockerServerConfig,
) -> anyhow::Result<SshChannel> {
    let private_key = state.master_key.decrypt(&config.private_key_enc)?;
    let runtime = sshfiles::materialize(
        &state.config.state_dir,
        target_id,
        &private_key,
        config.host_key.as_deref(),
    )?;
    Ok(SshChannel {
        user: config.ssh_user.clone(),
        host: config.host.clone(),
        port: config.port,
        key_path: runtime.key_path,
        known_hosts_path: runtime.known_hosts_path,
        control_dir: runtime.control_dir,
    })
}
