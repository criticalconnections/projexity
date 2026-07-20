//! Kubernetes deploy provider (M6).
//!
//! Renders Namespace/Deployment/Service/Ingress from a `ReleaseSpec` and
//! applies them with server-side apply (field manager `projexity`), pinning
//! images. Rollout is delegated to the Deployment controller
//! (`RollingUpdate maxUnavailable=0` + optional readinessProbe); a watch
//! surfaces progress. This mirrors the Docker provider's semantics without
//! reimplementing the choreography — Kubernetes does it natively.

pub mod client;
pub mod provider;
pub mod render;

/// Field manager name used for server-side apply.
pub const FIELD_MANAGER: &str = "projexity";

/// Labels stamped on every resource — the ownership protocol shared with the
/// Docker path. `destroy`/reconcile find resources by these.
pub const LABEL_MANAGED: &str = "app.kubernetes.io/managed-by";
pub const LABEL_APP: &str = "projexity.io/app";
pub const LABEL_RELEASE: &str = "projexity.io/release";

pub use client::{connect, validate, ClusterInfo, K8sConfig};
pub use provider::K8sProvider;
