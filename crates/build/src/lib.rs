//! Build pipeline: git ref → container image (M3).
//!
//! Flow: shallow clone on the control plane (GitHub App installation token
//! passed via `-c http.extraheader`, never in the URL — URLs leak into error
//! messages), resolve the exact SHA, detect a build plan, tar the context
//! (honoring .dockerignore, size-capped), and stream it to the destination
//! builder — the target server's own BuildKit for Docker targets.

pub mod plan;

/// Default cap on the streamed build context. Users *will* have 2GB of
/// node_modules committed; fail with a clear error instead of an SSH stall.
pub const DEFAULT_CONTEXT_LIMIT_BYTES: u64 = 500 * 1024 * 1024;
