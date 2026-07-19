//! Projexity core domain types.
//!
//! This crate defines the vocabulary shared by every other crate: entity IDs,
//! the immutable [`release::ReleaseSpec`], build/deployment state machines, and
//! the [`provider::DeployTarget`] / [`provider::Builder`] traits that the
//! Docker and Kubernetes providers implement.
//!
//! It deliberately has no I/O dependencies (no sqlx, no axum, no bollard) so
//! that domain logic stays testable in isolation.

pub mod error;
pub mod event;
pub mod id;
pub mod logs;
pub mod provider;
pub mod release;
pub mod state;

pub use error::{BuildError, DeployError, LogError, TargetError};
pub use event::{DeployEvent, EventSink};
pub use id::{AppRef, BuildId, DeploymentId, ProjectId, ReleaseId, TargetId, UserId};
pub use logs::{LogLine, LogOpts, LogStreamKind};
pub use provider::{
    AppStatus, Builder, BuiltImage, DeployOutcome, DeployTarget, ImageDestination, ImageSource,
    TargetInfo,
};
pub use release::{
    DeployPolicy, DomainSpec, EnvPair, HealthSpec, ImageRef, PortSpec, ReleaseSpec, ResourceSpec,
    SealedEnv,
};
pub use state::{BuildStatus, DeploymentStatus, TargetStatus};
