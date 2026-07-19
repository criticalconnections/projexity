//! GitHub App integration routes.
//!
//! Flow: dashboard → GET /github/manifest → browser POSTs the manifest to
//! github.com → GitHub redirects to /github/setup/callback?code= → we convert
//! the code into app credentials and store them encrypted → user installs the
//! app → GitHub redirects to /github/install/callback?installation_id= →
//! pushes start arriving at /webhooks/github.

use axum::{
    extract::{Query, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use projexity_db::github as gh_db;
use serde::{Deserialize, Serialize};

use crate::auth::CurrentUser;
use crate::state::AppState;

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(?e, "internal error");
    err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

#[derive(Debug, Serialize)]
pub struct GithubStatus {
    pub configured: bool,
    pub app_slug: Option<String>,
    pub app_url: Option<String>,
    pub installations: Vec<gh_db::Installation>,
    /// The public URL webhooks will be delivered to — surfaced so users can
    /// spot "localhost won't receive webhooks" immediately.
    pub webhook_url: String,
    pub public_url_is_local: bool,
}

pub async fn status(
    _user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<GithubStatus>, Response> {
    let app = gh_db::get_app(&state.pool).await.map_err(internal)?;
    let installations = gh_db::list_installations(&state.pool)
        .await
        .map_err(internal)?;
    let public = &state.config.public_url;
    Ok(Json(GithubStatus {
        configured: app.is_some(),
        app_slug: app.as_ref().map(|a| a.slug.clone()),
        app_url: app.as_ref().map(|a| a.html_url.clone()),
        installations,
        webhook_url: format!("{public}/api/v1/webhooks/github"),
        public_url_is_local: public.contains("localhost") || public.contains("127.0.0.1"),
    }))
}

#[derive(Debug, Serialize)]
pub struct ManifestResponse {
    /// Where the browser should POST the manifest.
    pub action_url: String,
    /// The JSON to submit in the `manifest` form field.
    pub manifest: serde_json::Value,
}

/// The app manifest the dashboard submits to GitHub. Each instance registers
/// its own app, so self-hosters never share credentials with anyone.
pub async fn manifest(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ManifestResponse>, Response> {
    let public = state.config.public_url.trim_end_matches('/');
    // App names are globally unique on GitHub; suffix with a short tag.
    let suffix: String = uuid::Uuid::new_v4().simple().to_string()[..6].to_string();
    let manifest = serde_json::json!({
        "name": format!("Projexity {suffix}"),
        "url": public,
        "hook_attributes": { "url": format!("{public}/api/v1/webhooks/github") },
        "redirect_url": format!("{public}/api/v1/github/setup/callback"),
        "setup_url": format!("{public}/api/v1/github/install/callback"),
        "public": false,
        "default_permissions": { "contents": "read", "metadata": "read" },
        "default_events": ["push"],
    });
    let _ = user;
    Ok(Json(ManifestResponse {
        action_url: "https://github.com/settings/apps/new".to_string(),
        manifest,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SetupCallback {
    pub code: String,
}

/// GitHub redirects the browser here after the user approves app creation.
pub async fn setup_callback(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(q): Query<SetupCallback>,
) -> Result<Redirect, Response> {
    let conv = projexity_github::app::convert_manifest_code(&q.code)
        .await
        .map_err(internal)?;
    let pem_enc = state
        .master_key
        .encrypt(conv.pem.as_bytes())
        .map_err(internal)?;
    let secret_enc = state
        .master_key
        .encrypt(conv.webhook_secret.as_bytes())
        .map_err(internal)?;
    gh_db::save_app(
        &state.pool,
        user.id,
        conv.id,
        &conv.slug,
        &conv.html_url,
        &conv.client_id,
        &pem_enc,
        &secret_enc,
    )
    .await
    .map_err(internal)?;
    tracing::info!(app = %conv.slug, "GitHub App registered");
    // Straight into the install step: connecting without installing is a
    // dead end users shouldn't be able to wander into.
    Ok(Redirect::to(&format!(
        "{}/installations/new",
        conv.html_url
    )))
}

#[derive(Debug, Deserialize)]
pub struct InstallCallback {
    pub installation_id: Option<i64>,
}

/// GitHub redirects here after the app is installed on an account.
pub async fn install_callback(
    _user: CurrentUser,
    State(state): State<AppState>,
    Query(q): Query<InstallCallback>,
) -> Result<Redirect, Response> {
    if let Some(installation_id) = q.installation_id {
        gh_db::upsert_installation(&state.pool, installation_id, "")
            .await
            .map_err(internal)?;
        tracing::info!(installation_id, "GitHub App installation recorded");
    }
    Ok(Redirect::to("/settings?github=connected"))
}

#[derive(Debug, Serialize)]
pub struct ReposResponse {
    pub connected: bool,
    pub repos: Vec<projexity_github::app::RepoInfo>,
}

pub async fn repos(
    _user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ReposResponse>, Response> {
    let Some(app) = gh_db::get_app(&state.pool).await.map_err(internal)? else {
        return Ok(Json(ReposResponse {
            connected: false,
            repos: vec![],
        }));
    };
    let installations = gh_db::list_installations(&state.pool)
        .await
        .map_err(internal)?;
    let pem = String::from_utf8(state.master_key.decrypt(&app.pem_enc).map_err(internal)?)
        .map_err(|e| internal(e.into()))?;

    let mut repos = Vec::new();
    for inst in &installations {
        match projexity_github::app::installation_token(app.app_id, &pem, inst.installation_id)
            .await
        {
            Ok(token) => match projexity_github::app::list_installation_repos(&token).await {
                Ok(mut r) => repos.append(&mut r),
                Err(e) => tracing::warn!(?e, "repo list failed"),
            },
            Err(e) => tracing::warn!(?e, "installation token failed"),
        }
    }
    repos.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(Json(ReposResponse {
        connected: true,
        repos,
    }))
}

/// Push webhook: HMAC-verified, no session auth. A push to a project's
/// configured branch starts a deployment; pushes mid-deploy are skipped (the
/// next push, or a manual deploy, picks up the latest commit).
pub async fn webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    RawQuery(_q): RawQuery,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, Response> {
    let Some(app) = gh_db::get_app(&state.pool).await.map_err(internal)? else {
        return Err(err(StatusCode::NOT_FOUND, "no GitHub App configured"));
    };
    let secret = state
        .master_key
        .decrypt(&app.webhook_secret_enc)
        .map_err(internal)?;
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !projexity_github::webhook::verify_signature(&secret, &body, signature) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad webhook signature"));
    }

    let event = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| internal(e.into()))?;

    match event {
        "push" => handle_push(&state, &payload).await.map_err(internal),
        "installation" => {
            let action = payload["action"].as_str().unwrap_or("");
            let id = payload["installation"]["id"].as_i64().unwrap_or(0);
            let login = payload["installation"]["account"]["login"]
                .as_str()
                .unwrap_or("");
            if id != 0 {
                match action {
                    "deleted" => gh_db::delete_installation(&state.pool, id)
                        .await
                        .map_err(internal)?,
                    _ => gh_db::upsert_installation(&state.pool, id, login)
                        .await
                        .map_err(internal)?,
                }
            }
            Ok(Json(serde_json::json!({ "ok": true })))
        }
        _ => Ok(Json(serde_json::json!({ "ok": true, "ignored": event }))),
    }
}

async fn handle_push(
    state: &AppState,
    payload: &serde_json::Value,
) -> anyhow::Result<Json<serde_json::Value>> {
    let git_ref = payload["ref"].as_str().unwrap_or("");
    let Some(branch) = git_ref.strip_prefix("refs/heads/") else {
        return Ok(Json(
            serde_json::json!({ "ok": true, "ignored": "non-branch ref" }),
        ));
    };
    let owner = payload["repository"]["owner"]["login"]
        .as_str()
        .or_else(|| payload["repository"]["owner"]["name"].as_str())
        .unwrap_or("");
    let name = payload["repository"]["name"].as_str().unwrap_or("");
    let head = payload["head_commit"]["id"].as_str().unwrap_or("");

    let matching = projexity_db::projects::find_by_repo(&state.pool, owner, name, branch).await?;
    let mut started = Vec::new();
    let mut skipped = Vec::new();
    for p in &matching {
        match crate::deploys::start_deploy(state, p, "deploy").await {
            Ok(Some(d)) => {
                tracing::info!(project = %p.slug, deployment = %d.id, commit = head,
                    "push-triggered deployment started");
                started.push(p.slug.clone());
            }
            Ok(None) => {
                tracing::info!(project = %p.slug, "push skipped — deployment already in flight");
                skipped.push(p.slug.clone());
            }
            Err(e) => {
                tracing::warn!(project = %p.slug, ?e, "push-triggered deploy failed to start")
            }
        }
    }
    Ok(Json(serde_json::json!({
        "ok": true,
        "started": started,
        "skipped": skipped,
    })))
}
