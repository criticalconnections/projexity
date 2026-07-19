//! Transport abstraction between the control plane and a server.
//!
//! MVP is agentless: [`SshChannel`] drives the system `ssh` binary with
//! connection multiplexing (ControlMaster) and a per-target known_hosts file
//! (`StrictHostKeyChecking=accept-new` = trust-on-first-use, then pinned).
//! A future agent implements the same [`NodeChannel`] trait over an outbound
//! websocket — nothing above this module may know SSH exists.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// A channel to one server. Implementations must be safe to share and must
/// survive reconnects transparently where possible.
#[async_trait]
pub trait NodeChannel: Send + Sync {
    /// Run a command on the server, returning exit code and output.
    /// Never pass secrets on command lines — they are visible in `ps`.
    async fn exec(&self, cmd: &str) -> anyhow::Result<ExecOutput>;

    /// Stream `contents` to a remote command's stdin (used to write files
    /// without putting their contents on a command line).
    async fn exec_with_stdin(&self, cmd: &str, stdin: &[u8]) -> anyhow::Result<ExecOutput>;
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

/// Single-quote a string for POSIX sh.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[derive(Debug, Clone)]
pub struct SshChannel {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// Private key file, mode 0600, materialized by the caller.
    pub key_path: PathBuf,
    /// Per-target known_hosts: empty on first connect (TOFU), pinned after.
    pub known_hosts_path: PathBuf,
    /// Directory for ControlMaster sockets.
    pub control_dir: PathBuf,
}

impl SshChannel {
    fn command(&self) -> Command {
        let mut c = Command::new("ssh");
        c.arg("-F")
            .arg("none")
            .arg("-i")
            .arg(&self.key_path)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                self.known_hosts_path.display()
            ))
            .arg("-o")
            .arg("StrictHostKeyChecking=accept-new")
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg("-o")
            .arg("ServerAliveInterval=15")
            .arg("-o")
            .arg("ServerAliveCountMax=4")
            .arg("-o")
            .arg("ControlMaster=auto")
            .arg("-o")
            .arg(format!("ControlPath={}/%C", self.control_dir.display()))
            .arg("-o")
            .arg("ControlPersist=60")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("{}@{}", self.user, self.host));
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        c
    }

    async fn run(&self, cmd: &str, stdin: Option<&[u8]>) -> anyhow::Result<ExecOutput> {
        let mut command = self.command();
        command.arg(cmd);
        if stdin.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|e| {
            anyhow::anyhow!("failed to start ssh (is OpenSSH installed on the control plane?): {e}")
        })?;
        if let Some(bytes) = stdin {
            let mut handle = child.stdin.take().expect("stdin piped");
            handle.write_all(bytes).await?;
            handle.shutdown().await?;
            drop(handle);
        }
        let out = child.wait_with_output().await?;
        Ok(ExecOutput {
            exit_code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[async_trait]
impl NodeChannel for SshChannel {
    async fn exec(&self, cmd: &str) -> anyhow::Result<ExecOutput> {
        self.run(cmd, None).await
    }

    async fn exec_with_stdin(&self, cmd: &str, stdin: &[u8]) -> anyhow::Result<ExecOutput> {
        self.run(cmd, Some(stdin)).await
    }
}

/// Turn raw ssh stderr into a message a user can act on. SSH failures are
/// connection problems, not "your server is broken" — say which.
pub fn classify_ssh_error(stderr: &str) -> String {
    let s = stderr.trim();
    if s.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        || s.contains("Host key verification failed")
    {
        "The server's SSH host key changed — this usually means the server was reinstalled. \
         Remove and re-add the target to trust the new key (or investigate if unexpected)."
            .to_string()
    } else if s.contains("Permission denied") {
        "SSH authentication failed — make sure you added the key to authorized_keys for this \
         user, then try again."
            .to_string()
    } else if s.contains("Connection refused") {
        "Connection refused — is SSH running on that port?".to_string()
    } else if s.contains("Connection timed out")
        || s.contains("Operation timed out")
        || s.contains("timed out")
    {
        "Connection timed out — check the address and any firewalls between us and the server."
            .to_string()
    } else if s.contains("Could not resolve hostname") {
        "Could not resolve that hostname — check for typos.".to_string()
    } else if s.contains("No route to host") {
        "No route to host — the address looks unreachable from here.".to_string()
    } else if s.is_empty() {
        "SSH connection failed for an unknown reason.".to_string()
    } else {
        // Last resort: the raw error, first line only.
        s.lines().next().unwrap_or(s).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("with 'quote'"), r"'with '\''quote'\'''");
    }

    #[test]
    fn classifies_common_errors() {
        assert!(classify_ssh_error("x: Permission denied (publickey).").contains("authorized_keys"));
        assert!(classify_ssh_error("connect: Connection refused").contains("refused"));
        assert!(
            classify_ssh_error("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!")
                .contains("reinstalled")
        );
    }
}
