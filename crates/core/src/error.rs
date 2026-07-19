use thiserror::Error;

/// Errors from target connectivity / capability probing.
#[derive(Debug, Error)]
pub enum TargetError {
    #[error("target unreachable: {0}")]
    Unreachable(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("target misconfigured: {0}")]
    Misconfigured(String),
    #[error("unsupported target capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors during deploy choreography. `Transport` errors are retryable and
/// must never be presented as an application failure ("your app is broken")
/// when the truth is "we lost the connection to your server".
#[derive(Debug, Error)]
pub enum DeployError {
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("image unavailable: {0}")]
    ImageUnavailable(String),
    #[error("release did not become healthy: {0}")]
    HealthGateFailed(String),
    #[error("traffic cutover failed: {0}")]
    CutoverFailed(String),
    #[error("deploy canceled")]
    Canceled,
    #[error("{0}")]
    Provider(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl DeployError {
    /// Transport-class errors should be retried at the step level before the
    /// deployment is failed.
    pub fn is_retryable(&self) -> bool {
        matches!(self, DeployError::Transport(_))
    }
}

/// Errors turning a git ref into an image.
#[derive(Debug, Error)]
pub enum BuildError {
    #[error("clone failed: {0}")]
    Clone(String),
    #[error("no build plan: {0}")]
    PlanDetection(String),
    #[error("build context too large: {actual_bytes} bytes (limit {limit_bytes})")]
    ContextTooLarge { actual_bytes: u64, limit_bytes: u64 },
    #[error("build failed: {0}")]
    Build(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("build canceled")]
    Canceled,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum LogError {
    #[error("log source gone: {0}")]
    SourceGone(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
