//! Server onboarding: fact-gathering preflight + idempotent bootstrap.
//!
//! Every step is check-then-act so "Repair" (and version upgrades) can simply
//! re-run the whole sequence. Implemented in M1.

/// Facts gathered before mutating anything. Failures at this stage produce
/// specific, actionable errors ("port 80 is held by nginx (pid 312)") — this
/// is where onboarding support tickets go to die.
#[derive(Debug, Clone, Default)]
pub struct ServerFacts {
    pub os: String,
    pub arch: String,
    pub docker_version: Option<String>,
    pub port_80_free: bool,
    pub port_443_free: bool,
    pub disk_free_bytes: u64,
    pub memory_total_bytes: u64,
    pub has_sudo: bool,
}

/// Bootstrap script version stamped into /etc/projexity/server.json; bumping
/// it marks connected servers as needing a re-run.
pub const BOOTSTRAP_VERSION: u32 = 1;
