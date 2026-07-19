use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::auth::{self, CurrentUser};
use crate::routes_targets;
use crate::state::AppState;

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
        .route("/deployments/{id}/logs/stream", get(deployment_logs_stream));

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

/// Live log stream for a deployment (SSE).
///
/// M0 stub: emits keep-alives only, establishing the endpoint shape the
/// dashboard codes against. M2/M3 replay persisted `deployment_logs` from the
/// `Last-Event-ID` cursor, then live-tail from the worker's broadcast channel.
async fn deployment_logs_stream(
    _user: CurrentUser,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = futures::stream::pending::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
