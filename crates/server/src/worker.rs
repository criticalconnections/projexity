//! In-process background worker: claims jobs from the Postgres queue
//! (`FOR UPDATE SKIP LOCKED` + lease heartbeat) and dispatches by kind.

use std::time::Duration;

use projexity_db::jobs::{self, Job};

use crate::state::AppState;
use crate::{jobs_deploy, jobs_setup, jobs_template};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const HOUSEKEEPING_INTERVAL: Duration = Duration::from_secs(3600);

pub async fn run(state: AppState) {
    tracing::info!("worker started");
    let mut housekeeping = tokio::time::interval(HOUSEKEEPING_INTERVAL);

    loop {
        tokio::select! {
            _ = housekeeping.tick() => {
                run_housekeeping(&state).await;
            }
            claimed = claim_next(&state) => {
                match claimed {
                    Some(job) => execute(&state, job).await,
                    // Queue empty: back off before polling again.
                    None => tokio::time::sleep(POLL_INTERVAL).await,
                }
            }
        }
    }
}

async fn claim_next(state: &AppState) -> Option<Job> {
    match jobs::claim(&state.pool).await {
        Ok(job) => job,
        Err(err) => {
            tracing::error!(?err, "job claim failed");
            None
        }
    }
}

async fn execute(state: &AppState, job: Job) {
    tracing::info!(job_id = %job.id, kind = %job.kind, attempt = job.attempts, "job started");

    // Long jobs (bootstrap installs Docker; builds take minutes) must keep
    // their lease alive or another worker will reclaim them mid-flight.
    let heartbeat = {
        let pool = state.pool.clone();
        let job_id = job.id;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
            tick.tick().await; // immediate first tick — skip
            loop {
                tick.tick().await;
                match jobs::heartbeat(&pool, job_id).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(%job_id, "lost job lease");
                        break;
                    }
                    Err(err) => tracing::error!(%job_id, ?err, "heartbeat failed"),
                }
            }
        })
    };

    let result: anyhow::Result<()> = match job.kind.as_str() {
        "setup_server" => jobs_setup::run(state, job.payload.clone()).await,
        "run_deployment" => jobs_deploy::run(state, job.payload.clone()).await,
        "destroy_app" => jobs_deploy::destroy(state, job.payload.clone()).await,
        "install_template" => jobs_template::install(state, job.payload.clone()).await,
        "uninstall_template" => jobs_template::uninstall(state, job.payload.clone()).await,
        other => Err(anyhow::anyhow!("unknown job kind: {other}")),
    };

    heartbeat.abort();

    let outcome = match result {
        Ok(()) => jobs::succeed(&state.pool, job.id).await,
        Err(err) => {
            tracing::warn!(job_id = %job.id, ?err, "job failed");
            jobs::fail(&state.pool, job.id, &format!("{err:#}")).await
        }
    };
    if let Err(err) = outcome {
        tracing::error!(job_id = %job.id, ?err, "failed to record job outcome");
    }
}

async fn run_housekeeping(state: &AppState) {
    if let Err(err) = projexity_db::sessions::prune_expired(&state.pool).await {
        tracing::error!(?err, "session prune failed");
    }
    if let Err(err) = jobs::prune(&state.pool, chrono::Duration::days(7)).await {
        tracing::error!(?err, "job prune failed");
    }
}
