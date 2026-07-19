use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use projexity_core::ReleaseId;
use projexity_db::{deployments, env_vars, projects};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::release::{generated_domain, EncEnvPair, ReleaseSnapshot};
use crate::state::AppState;

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(?e, "internal error");
    err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub target_id: Option<Uuid>,
    pub image: Option<String>,
    pub container_port: i32,
    pub domains: Vec<String>,
    pub latest_deployment: Option<deployments::Deployment>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn to_response(state: &AppState, p: &projects::Project) -> anyhow::Result<ProjectResponse> {
    let domains = projects::domains(&state.pool, p.id).await?;
    let latest = deployments::list_for_project(&state.pool, p.id, 1)
        .await?
        .into_iter()
        .next();
    Ok(ProjectResponse {
        id: p.id,
        name: p.name.clone(),
        slug: p.slug.clone(),
        target_id: p.target_id,
        image: p.image.clone(),
        container_port: p.container_port,
        domains,
        latest_deployment: latest,
        created_at: p.created_at,
    })
}

fn slugify(name: &str) -> String {
    let mut slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub target_id: Uuid,
    /// Prebuilt image ref (`nginx:latest`). Git-based projects arrive in M3+.
    pub image: String,
    #[serde(default = "default_port")]
    pub container_port: i32,
}

fn default_port() -> i32 {
    80
}

pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<CreateProject>,
) -> Result<Json<ProjectResponse>, Response> {
    let name = req.name.trim();
    let slug = slugify(name);
    if slug.is_empty() {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "give the project a name",
        ));
    }
    if req.image.trim().is_empty() {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "an image is required",
        ));
    }
    if !(1..=65535).contains(&req.container_port) {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "invalid port"));
    }
    let target = projexity_db::targets::find_for_user(&state.pool, user.id, req.target_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::UNPROCESSABLE_ENTITY, "unknown target"))?;
    if target.status != "ready" {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "that server isn't ready yet — finish its setup first",
        ));
    }

    let project = projects::create(
        &state.pool,
        user.id,
        target.id,
        name,
        &slug,
        Some(req.image.trim()),
        req.container_port,
    )
    .await
    .map_err(internal)?
    .ok_or_else(|| {
        err(
            StatusCode::CONFLICT,
            "a project with this name already exists",
        )
    })?;

    // Instant free domain via sslip.io (or a deterministic name for
    // hostname-based targets).
    let config = target.docker_config().map_err(internal)?;
    let domain = generated_domain(&slug, &config.host);
    projects::add_domain(&state.pool, project.id, &domain, true)
        .await
        .map_err(internal)?;

    Ok(Json(to_response(&state, &project).await.map_err(internal)?))
}

pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectResponse>>, Response> {
    let rows = projects::list_for_user(&state.pool, user.id)
        .await
        .map_err(internal)?;
    let mut out = Vec::with_capacity(rows.len());
    for p in &rows {
        out.push(to_response(&state, p).await.map_err(internal)?);
    }
    Ok(Json(out))
}

async fn load(
    state: &AppState,
    user: &CurrentUser,
    id: Uuid,
) -> Result<projects::Project, Response> {
    projects::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "project not found"))
}

pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, Response> {
    let p = load(&state, &user, id).await?;
    Ok(Json(to_response(&state, &p).await.map_err(internal)?))
}

pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Response> {
    let p = load(&state, &user, id).await?;
    // Tear down the app's containers/routes in the background, then drop rows.
    if let Some(target_id) = p.target_id {
        projexity_db::jobs::enqueue(
            &state.pool,
            "destroy_app",
            serde_json::json!({ "target_id": target_id, "slug": p.slug }),
            None,
        )
        .await
        .map_err(internal)?;
    }
    projects::delete(&state.pool, user.id, id)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvVarBody {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub is_build_time: bool,
}

pub async fn get_env(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<EnvVarBody>>, Response> {
    let p = load(&state, &user, id).await?;
    let rows = env_vars::list(&state.pool, p.id).await.map_err(internal)?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let ct = String::from_utf8(r.value_ciphertext).map_err(|e| internal(e.into()))?;
        let value = String::from_utf8(state.master_key.decrypt(&ct).map_err(internal)?)
            .map_err(|e| internal(e.into()))?;
        out.push(EnvVarBody {
            key: r.key,
            value,
            is_build_time: r.is_build_time,
        });
    }
    Ok(Json(out))
}

pub async fn put_env(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(vars): Json<Vec<EnvVarBody>>,
) -> Result<StatusCode, Response> {
    let p = load(&state, &user, id).await?;
    let mut rows = Vec::with_capacity(vars.len());
    for v in &vars {
        let key = v.key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) || key.contains('=') {
            return Err(err(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("invalid env var name: {key:?}"),
            ));
        }
        rows.push(env_vars::EnvVarRow {
            key: key.to_string(),
            value_ciphertext: state
                .master_key
                .encrypt(v.value.as_bytes())
                .map_err(internal)?
                .into_bytes(),
            is_build_time: v.is_build_time,
        });
    }
    env_vars::replace_all(&state.pool, p.id, &rows)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Trigger a deploy of the project's configured image.
pub async fn deploy(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<deployments::Deployment>, Response> {
    let p = load(&state, &user, id).await?;
    let image = p.image.clone().ok_or_else(|| {
        err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "project has no image configured",
        )
    })?;

    let env_rows = env_vars::list(&state.pool, p.id).await.map_err(internal)?;
    let env: Vec<EncEnvPair> = env_rows
        .into_iter()
        .filter(|r| !r.is_build_time)
        .map(|r| {
            Ok(EncEnvPair {
                key: r.key,
                value_enc: String::from_utf8(r.value_ciphertext)?,
            })
        })
        .collect::<anyhow::Result<_>>()
        .map_err(internal)?;
    let domains = projects::domains(&state.pool, p.id)
        .await
        .map_err(internal)?;

    let snapshot = ReleaseSnapshot {
        release_id: ReleaseId::new(),
        app_slug: p.slug.clone(),
        image,
        container_port: p.container_port as u16,
        domains,
        env,
    };

    let deployment = deployments::create(
        &state.pool,
        p.id,
        "deploy",
        &serde_json::to_value(&snapshot).map_err(|e| internal(e.into()))?,
    )
    .await
    .map_err(internal)?
    .ok_or_else(|| {
        err(
            StatusCode::CONFLICT,
            "a deployment is already in progress for this project",
        )
    })?;

    projexity_db::jobs::enqueue(
        &state.pool,
        "run_deployment",
        serde_json::json!({ "deployment_id": deployment.id }),
        None,
    )
    .await
    .map_err(internal)?;

    Ok(Json(deployment))
}

pub async fn list_deployments(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<deployments::Deployment>>, Response> {
    let p = load(&state, &user, id).await?;
    let rows = deployments::list_for_project(&state.pool, p.id, 50)
        .await
        .map_err(internal)?;
    Ok(Json(rows))
}

pub async fn list_all_deployments(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<deployments::Deployment>>, Response> {
    let rows = deployments::list_for_user(&state.pool, user.id, 100)
        .await
        .map_err(internal)?;
    Ok(Json(rows))
}

pub async fn get_deployment(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<deployments::Deployment>, Response> {
    let d = deployments::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "deployment not found"))?;
    Ok(Json(d))
}
