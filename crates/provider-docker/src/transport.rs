//! Transport abstraction between the control plane and a server.
//!
//! MVP implementation (M1) will be agentless SSH via the `openssh` crate; a
//! future agent implements the same trait over an outbound websocket.

use async_trait::async_trait;

/// A channel to one server. Implementations must be safe to share and must
/// survive reconnects transparently where possible.
#[async_trait]
pub trait NodeChannel: Send + Sync {
    /// Run a command on the server, returning (exit_code, stdout, stderr).
    /// Never pass secrets on command lines — they are visible in `ps`.
    async fn exec(&self, cmd: &str) -> anyhow::Result<ExecOutput>;

    /// Write a file on the server with the given mode.
    async fn put_file(&self, path: &str, contents: &[u8], mode: u32) -> anyhow::Result<()>;

    /// Path to a local unix socket forwarding the server's docker socket.
    /// (bollard connects to this.)
    async fn docker_socket(&self) -> anyhow::Result<std::path::PathBuf>;

    /// Local address forwarding the server's Caddy admin API (127.0.0.1:2019).
    async fn caddy_admin_addr(&self) -> anyhow::Result<std::net::SocketAddr>;
}

#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecOutput {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}
