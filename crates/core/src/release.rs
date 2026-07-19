use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::{AppRef, ReleaseId};

/// Immutable, fully-resolved description of one release.
///
/// This is the portability boundary: everything here must be expressible on
/// BOTH the Docker and Kubernetes providers, or it doesn't belong here.
/// Provider-specific knobs live in the target's own config, not in the spec.
///
/// Rollback is `deploy()` with an older release's spec — the spec pins the
/// image by digest and snapshots env, so replaying it reproduces the release
/// exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSpec {
    pub app: AppRef,
    pub release_id: ReleaseId,
    pub image: ImageRef,
    pub env: SealedEnv,
    pub ports: Vec<PortSpec>,
    pub health: HealthSpec,
    pub resources: ResourceSpec,
    pub replicas: u32,
    pub domains: Vec<DomainSpec>,
    pub deploy_policy: DeployPolicy,
}

/// A container image reference, optionally pinned by digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    /// e.g. `pjx/myapp:release-01hx...` or `nginx:1.27`
    pub name: String,
    /// `sha256:...` — set after a build or first pull; deploys should pin it.
    pub digest: Option<String>,
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.digest {
            Some(d) => write!(f, "{}@{}", self.name, d),
            None => write!(f, "{}", self.name),
        }
    }
}

/// One env var. Values are handled inside [`SealedEnv`] so they never appear
/// in Debug output or logs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvPair {
    pub key: String,
    pub value: String,
}

impl fmt::Debug for EnvPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}=***", self.key)
    }
}

/// Decrypted env for a release, kept opaque so secret values can't leak via
/// `Debug`/`Display`. Constructed only at the injection point (container
/// create body / K8s Secret render).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SealedEnv(Vec<EnvPair>);

impl SealedEnv {
    pub fn new(pairs: Vec<EnvPair>) -> Self {
        Self(pairs)
    }

    pub fn pairs(&self) -> &[EnvPair] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Secret values (length >= 6) to mask in log output, GitHub-Actions style.
    pub fn redactable_values(&self) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .map(|p| p.value.as_str())
            .filter(|v| v.len() >= 6)
    }
}

impl fmt::Debug for SealedEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SealedEnv({} vars)", self.0.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortProtocol {
    Http,
    Tcp,
}

/// A port the app listens on inside the container. No host ports — the proxy
/// (Caddy) or Service reaches the app over the container network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub container_port: u16,
    pub protocol: PortProtocol,
}

/// Health checking, translated to a Docker HEALTHCHECK + deploy-time gate on
/// the Docker path and a readinessProbe on Kubernetes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HealthSpec {
    Http {
        path: String,
        port: u16,
        initial_delay_secs: u32,
        period_secs: u32,
        timeout_secs: u32,
        failure_threshold: u32,
    },
    Tcp {
        port: u16,
        initial_delay_secs: u32,
        period_secs: u32,
        timeout_secs: u32,
        failure_threshold: u32,
    },
    /// No probe: the deploy gate only requires the container to stay running
    /// through a short settle delay.
    #[default]
    None,
}

/// Resource limits both providers can express.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub cpu_millicores: Option<u32>,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainSpec {
    pub hostname: String,
    /// Generated `<slug>.<base>` domains are managed by us; custom domains
    /// are user-owned (and drive Caddy on-demand TLS).
    pub is_generated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployPolicy {
    /// Grace period for the old release to drain after traffic cutover.
    pub drain_seconds: u32,
    /// How long the new release has to become healthy before the deploy fails.
    pub health_gate_timeout_seconds: u32,
}

impl Default for DeployPolicy {
    fn default() -> Self {
        Self {
            drain_seconds: 30,
            health_gate_timeout_seconds: 90,
        }
    }
}
