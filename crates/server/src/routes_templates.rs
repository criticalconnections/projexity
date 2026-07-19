use std::collections::BTreeMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use projexity_db::template_deployments as td;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::state::AppState;
use crate::templates;

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(?e, "internal error");
    err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

pub async fn catalog(
    _user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<templates::CatalogEntry>>, Response> {
    Ok(Json(
        state.templates.iter().map(|t| t.catalog_entry()).collect(),
    ))
}

pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<td::TemplateDeployment>>, Response> {
    Ok(Json(
        td::list_for_user(&state.pool, user.id)
            .await
            .map_err(internal)?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub template_id: String,
    pub target_id: Uuid,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
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

pub async fn install(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<InstallRequest>,
) -> Result<Json<td::TemplateDeployment>, Response> {
    let template = state
        .templates
        .iter()
        .find(|t| t.id == req.template_id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown template"))?;

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
    let config = target.docker_config().map_err(internal)?;

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&template.name)
        .to_string();
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "give the app a name"));
    }

    let env = templates::resolve_env(template, &req.env)
        .map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()))?;
    let domains = templates::assign_domains(template, &slug, &config.host);

    let env_enc = state
        .master_key
        .encrypt(
            serde_json::to_vec(&env)
                .map_err(|e| internal(e.into()))?
                .as_slice(),
        )
        .map_err(internal)?;

    let deployment = td::create(
        &state.pool,
        user.id,
        target.id,
        &template.id,
        &name,
        &slug,
        &env_enc,
        &serde_json::to_value(&domains).map_err(|e| internal(e.into()))?,
    )
    .await
    .map_err(internal)?
    .ok_or_else(|| err(StatusCode::CONFLICT, "an app with this name already exists"))?;

    projexity_db::jobs::enqueue(
        &state.pool,
        "install_template",
        serde_json::json!({ "template_deployment_id": deployment.id }),
        None,
    )
    .await
    .map_err(internal)?;

    Ok(Json(deployment))
}

pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<td::TemplateDeployment>, Response> {
    td::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(internal)?
        .map(Json)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "app not found"))
}

pub async fn uninstall(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Response> {
    let d = td::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "app not found"))?;
    td::set_status_only(&state.pool, d.id, "removing")
        .await
        .map_err(internal)?;
    projexity_db::jobs::enqueue(
        &state.pool,
        "uninstall_template",
        serde_json::json!({ "template_deployment_id": d.id }),
        None,
    )
    .await
    .map_err(internal)?;
    Ok(StatusCode::ACCEPTED)
}
