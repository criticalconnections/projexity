//! SSE log streaming: deploy logs replay + live-tail from Postgres (resume
//! via Last-Event-ID), runtime logs streamed straight off the container.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    Json,
};
use futures::stream::Stream;
use futures::StreamExt;
use projexity_db::{deployment_logs, deployments};
use projexity_provider_docker::docker::DockerServer;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::routes_targets::channel_for;
use crate::state::AppState;

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// Deploy log stream: replays persisted lines from the Last-Event-ID cursor,
/// then live-tails until the deployment settles.
pub async fn deploy_logs(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let deployment = deployments::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "load deployment");
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "deployment not found"))?;

    let mut cursor: i64 = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let pool = state.pool.clone();
    let deployment_id = deployment.id;

    let stream = async_stream::stream! {
        loop {
            let rows = match deployment_logs::fetch_since(&pool, deployment_id, cursor, 500).await {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!(?e, "log fetch failed");
                    break;
                }
            };
            let empty = rows.is_empty();
            for row in rows {
                cursor = row.seq;
                yield Ok(Event::default()
                    .id(row.seq.to_string())
                    .event("log")
                    .data(serde_json::json!({
                        "seq": row.seq,
                        "stream": row.stream,
                        "ts": row.ts,
                        "text": row.text,
                    }).to_string()));
            }
            if empty {
                // No new lines: stop once the deployment has settled.
                let status = deployments::find(&pool, deployment_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|d| d.status);
                match status.as_deref() {
                    Some("pending") | Some("deploying") | Some("verifying") => {
                        tokio::time::sleep(Duration::from_millis(700)).await;
                    }
                    other => {
                        yield Ok(Event::default()
                            .event("end")
                            .data(other.unwrap_or("gone")));
                        break;
                    }
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Live runtime logs (docker logs -f) for a project's current container.
pub async fn runtime_logs(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let project = projexity_db::projects::find_for_user(&state.pool, user.id, id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "load project");
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "project not found"))?;
    let target_id = project
        .target_id
        .ok_or_else(|| err(StatusCode::UNPROCESSABLE_ENTITY, "project has no target"))?;
    let target = projexity_db::targets::find(&state.pool, target_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "load target");
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "target not found"))?;
    let config = target
        .docker_config()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    let channel = channel_for(&state, target.id, &config)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    let server = DockerServer { channel };

    let (lines, guard) = server
        .runtime_log_stream(&project.slug, 200)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    let stream = async_stream::stream! {
        // Keep the SSH forward alive for the lifetime of this stream.
        let _guard = guard;
        let mut lines = std::pin::pin!(lines);
        while let Some(text) = lines.next().await {
            yield Ok(Event::default().event("log").data(
                serde_json::json!({ "text": text }).to_string(),
            ));
        }
        yield Ok(Event::default().event("end").data("closed"));
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
