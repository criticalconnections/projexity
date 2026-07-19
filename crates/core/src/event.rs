use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::id::{DeploymentId, ReleaseId};

/// Progress events emitted by providers/builders during long-running work.
/// They feed the deployment event log (audit trail) and the dashboard's live
/// SSE stream. Both providers emit the SAME event vocabulary — the K8s
/// rollout watcher translates Deployment conditions into these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeployEvent {
    StepStarted { step: String },
    StepCompleted { step: String },
    Progress { message: String },
    HealthProbe { healthy: bool, detail: String },
    TrafficShifted { release_id: ReleaseId },
    Warning { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployEventEnvelope {
    pub deployment_id: DeploymentId,
    pub ts: DateTime<Utc>,
    pub event: DeployEvent,
}

/// Cheap-to-clone sink providers push events into. The receiving side (the
/// worker) persists envelopes and fans them out to SSE subscribers. Sending
/// never blocks provider progress; if the receiver is gone the event is
/// dropped (the deploy itself is the source of truth, events are telemetry).
#[derive(Debug, Clone)]
pub struct EventSink {
    deployment_id: DeploymentId,
    tx: mpsc::UnboundedSender<DeployEventEnvelope>,
}

impl EventSink {
    pub fn new(
        deployment_id: DeploymentId,
    ) -> (Self, mpsc::UnboundedReceiver<DeployEventEnvelope>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { deployment_id, tx }, rx)
    }

    pub fn emit(&self, event: DeployEvent) {
        let _ = self.tx.send(DeployEventEnvelope {
            deployment_id: self.deployment_id,
            ts: Utc::now(),
            event,
        });
    }

    pub fn step_started(&self, step: impl Into<String>) {
        self.emit(DeployEvent::StepStarted { step: step.into() });
    }

    pub fn step_completed(&self, step: impl Into<String>) {
        self.emit(DeployEvent::StepCompleted { step: step.into() });
    }

    pub fn progress(&self, message: impl Into<String>) {
        self.emit(DeployEvent::Progress {
            message: message.into(),
        });
    }
}
