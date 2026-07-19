use axum::{
    extract::State,
    routing::{get, post, put},
    Json, Router,
};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::state::AppState;
use crate::{auth, routes_github, routes_logs, routes_projects, routes_targets};

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/healthz", get(healthz))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route(
            "/targets",
            get(routes_targets::list).post(routes_targets::create),
        )
        .route(
            "/targets/{id}",
            get(routes_targets::get).delete(routes_targets::delete),
        )
        .route("/targets/{id}/check", post(routes_targets::check))
        .route("/targets/{id}/bootstrap", post(routes_targets::bootstrap))
        .route(
            "/projects",
            get(routes_projects::list).post(routes_projects::create),
        )
        .route(
            "/projects/{id}",
            get(routes_projects::get).delete(routes_projects::delete),
        )
        .route(
            "/projects/{id}/env",
            get(routes_projects::get_env).merge(put(routes_projects::put_env)),
        )
        .route("/projects/{id}/deploy", post(routes_projects::deploy))
        .route(
            "/projects/{id}/deployments",
            get(routes_projects::list_deployments),
        )
        .route(
            "/projects/{id}/runtime-logs/stream",
            get(routes_logs::runtime_logs),
        )
        .route("/deployments", get(routes_projects::list_all_deployments))
        .route("/deployments/{id}", get(routes_projects::get_deployment))
        .route(
            "/deployments/{id}/rollback",
            post(routes_projects::rollback),
        )
        .route(
            "/deployments/{id}/logs/stream",
            get(routes_logs::deploy_logs),
        )
        .route("/github/app", get(routes_github::status))
        .route("/github/manifest", get(routes_github::manifest))
        .route("/github/setup/callback", get(routes_github::setup_callback))
        .route(
            "/github/install/callback",
            get(routes_github::install_callback),
        )
        .route("/github/repos", get(routes_github::repos))
        .route("/webhooks/github", post(routes_github::webhook));

    // SPA: serve the built dashboard; unknown paths fall back to index.html
    // so client-side routing works on refresh.
    let dist = state.config.web_dist.clone();
    let spa = ServeDir::new(&dist).fallback(tower_http::services::ServeFile::new(format!(
        "{dist}/index.html"
    )));

    Router::new()
        .nest("/api/v1", api)
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn healthz(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": db_ok,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
