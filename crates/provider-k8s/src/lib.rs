//! Kubernetes deploy provider (M6).
//!
//! Renders Namespace/Deployment/Service/Ingress from a `ReleaseSpec` and
//! applies them with server-side apply (field manager `projexity`), pinning
//! images by digest. Rollout choreography is delegated to the Deployment
//! controller (`RollingUpdate maxUnavailable=0` + readinessProbe from
//! `HealthSpec`); a watch translates rollout conditions and pod events
//! (`ImagePullBackOff`, `CrashLoopBackOff`) into the same `DeployEvent`
//! stream the Docker provider emits.
//!
//! The `kube`/`k8s-openapi` dependencies land with the implementation to keep
//! scaffold builds fast.

/// Field manager name used for server-side apply.
pub const FIELD_MANAGER: &str = "projexity";
