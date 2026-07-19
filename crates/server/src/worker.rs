//! In-process background worker: claims jobs from the Postgres queue
//! (`FOR UPDATE SKIP LOCKED` + lease heartbeat) and dispatches by kind.
//!
//! Job kinds land with their milestones: `SetupServer` (M1), `RunDeployment`
//! (M2), `RunBuild` (M3). Housekeeping (session pruning, job pruning) runs on
//! a slow tick.

use std::time::Duration;

use projexity_db::jobs::{self, Job};

use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
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
    // Job kinds are dispatched here as milestones land: SetupServer (M1),
    // RunDeployment (M2), RunBuild (M3).
    let result: anyhow::Result<()> = Err(anyhow::anyhow!("unknown job kind: {}", job.kind));

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
