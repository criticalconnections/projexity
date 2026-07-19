//! Shallow clone on the control plane. Tokens (M4) are passed via
//! `-c http.extraheader`, never in the URL — URLs leak into error messages.

use std::path::Path;
use std::process::Stdio;

use projexity_core::BuildError;

pub struct CloneResult {
    pub sha: String,
    pub message: String,
}

/// Shallow-clone `repo_url`@`branch` into `dest`. `auth_header` is the value
/// for `http.extraheader` (e.g. `Authorization: basic <b64>`), if any.
pub async fn shallow_clone(
    repo_url: &str,
    branch: &str,
    dest: &Path,
    auth_header: Option<&str>,
) -> Result<CloneResult, BuildError> {
    let mut cmd = tokio::process::Command::new("git");
    if let Some(header) = auth_header {
        cmd.arg("-c").arg(format!("http.extraheader={header}"));
    }
    cmd.arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--branch")
        .arg(branch)
        .arg("--single-branch")
        .arg(repo_url)
        .arg(dest)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let out = cmd
        .output()
        .await
        .map_err(|e| BuildError::Clone(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let friendly = if stderr.contains("not found") || stderr.contains("does not exist") {
            "repository not found — is it public, and is the name spelled right?".to_string()
        } else if stderr.contains("Could not find remote branch") {
            format!("branch '{branch}' doesn't exist in this repository")
        } else {
            stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("clone failed")
                .to_string()
        };
        return Err(BuildError::Clone(friendly));
    }

    let sha = git_out(dest, &["rev-parse", "HEAD"]).await?;
    let message = git_out(dest, &["log", "-1", "--pretty=%s"]).await?;
    Ok(CloneResult { sha, message })
}

async fn git_out(dir: &Path, args: &[&str]) -> Result<String, BuildError> {
    let out = tokio::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .await
        .map_err(|e| BuildError::Clone(format!("git failed: {e}")))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
