use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{BuildError, DeployError, LogError, TargetError};
use crate::event::EventSink;
use crate::id::{AppRef, ReleaseId};
use crate::logs::{LogLine, LogOpts};
use crate::release::{ImageRef, ReleaseSpec};

/// Facts about a target gathered by `preflight()`. Architecture matters:
/// builds run where they'll execute, so arm64 targets get arm64 images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInfo {
    pub arch: String,
    pub os: String,
    pub engine_version: String,
    pub disk_free_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
}

/// How `ensure_image` should make the image available on the target.
pub enum ImageSource {
    /// The image was built on this target's daemon and is already present.
    AlreadyPresent,
    /// Pull from a registry (K8s path, or Docker path for public images).
    Registry { credentials: Option<RegistryAuth> },
    /// Stream a `docker save` tarball (fallback: rollback to a pruned image,
    /// air-gapped-ish targets).
    TarStream(Box<dyn tokio::io::AsyncRead + Send + Unpin>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployOutcome {
    pub release_id: ReleaseId,
    /// Provider-native handle (container id / Deployment name+namespace),
    /// persisted on the deployment row for status/log/destroy calls.
    pub provider_ref: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppHealth {
    Healthy,
    Unhealthy,
    Stopped,
    Unknown,
}

/// Live status, derived from reality (docker inspect / pod list), never from
/// our database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatus {
    pub health: AppHealth,
    pub running_release: Option<ReleaseId>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub removed_containers: Vec<String>,
    pub removed_images: Vec<String>,
    pub drift_notes: Vec<String>,
}

/// A concrete deploy target instance (one VPS, one cluster).
///
/// The trait is declarative: `deploy` means "make reality match this
/// ReleaseSpec". The Docker provider internalizes the imperative choreography
/// (start green, health-gate, cut traffic via Caddy, drain blue) behind this
/// surface; the K8s provider delegates it to a Deployment rollout and watches
/// conditions. Contract requirements for implementors:
///
/// - `deploy` MUST be idempotent and resumable: re-invoking with the same
///   `ReleaseSpec` after a crash finishes the deploy or no-ops. Deterministic
///   resource names (`pjx-<slug>-<release>`) are the mechanism.
/// - Every created resource MUST carry the managed labels
///   (`projexity.managed`, `projexity.app`, `projexity.release`) so
///   `destroy`/`reconcile` can find them, and the reconciler must never touch
///   anything without those labels — users run other things on these boxes.
/// - `cancel` is honored only before traffic cutover; after cutover the
///   deploy rolls forward.
#[async_trait]
pub trait DeployTarget: Send + Sync {
    /// Cheap connectivity + capability probe. Called before every deploy so
    /// "server died since last deploy" is a fast, specific error instead of a
    /// timeout minutes into a build.
    async fn preflight(&self) -> Result<TargetInfo, TargetError>;

    /// Make the target's image store contain `image`. Hides the registry
    /// question from everything above the trait.
    async fn ensure_image(
        &self,
        image: &ImageRef,
        source: ImageSource,
        events: &EventSink,
    ) -> Result<(), DeployError>;

    /// Converge the target to `release`. Emits progress into `events`.
    async fn deploy(
        &self,
        release: &ReleaseSpec,
        events: &EventSink,
        cancel: CancellationToken,
    ) -> Result<DeployOutcome, DeployError>;

    async fn status(&self, app: &AppRef) -> Result<AppStatus, DeployError>;

    /// Runtime (not build) logs. `opts.since_seq` enables resume after a
    /// dropped stream.
    async fn runtime_logs(
        &self,
        app: &AppRef,
        opts: LogOpts,
    ) -> Result<BoxStream<'static, Result<LogLine, LogError>>, LogError>;

    /// Remove all resources labeled with this app. Best-effort, idempotent.
    async fn destroy(&self, app: &AppRef) -> Result<(), DeployError>;

    /// Remove resources not belonging to `keep` (current + retained releases):
    /// crashed deploys' half-born containers, images beyond retention, build
    /// cache beyond budget.
    async fn reconcile(
        &self,
        app: &AppRef,
        keep: &[ReleaseId],
    ) -> Result<ReconcileReport, DeployError>;
}

/// Where a built image should end up.
pub enum ImageDestination<'a> {
    /// Build directly on the deploy target's daemon (Docker path MVP): the
    /// image materializes where it runs, no registry, correct arch for free.
    LoadOnTarget(&'a dyn DeployTarget),
    /// Push to a registry (K8s path; multi-server later).
    PushRegistry {
        image: ImageRef,
        auth: Option<RegistryAuth>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltImage {
    pub image: ImageRef,
    pub size_bytes: Option<u64>,
}

/// Source input for a build: an already-cloned, plan-resolved workdir,
/// packaged as a tar context by the build crate.
pub struct BuildInput {
    /// Human-readable description of the detected plan ("Dockerfile at ./",
    /// "Nixpacks: node") — always printed to the build log so "why did it
    /// think my app is PHP" is self-serviceable.
    pub plan_summary: String,
    /// Tar archive of the build context (Dockerfile included/generated).
    pub context_tar: Vec<u8>,
    /// Build-time env (explicitly non-secret bucket; users opt in knowing
    /// these bake into image layers).
    pub build_args: Vec<(String, String)>,
    pub image_tag: String,
}

/// Turns a build context into a container image at a destination. Split from
/// [`DeployTarget`] because building and running have different lifecycles,
/// failure modes, and eventually different machines (dedicated builder fleet).
#[async_trait]
pub trait Builder: Send + Sync {
    async fn build(
        &self,
        input: BuildInput,
        output: ImageDestination<'_>,
        events: &EventSink,
        cancel: CancellationToken,
    ) -> Result<BuiltImage, BuildError>;
}
