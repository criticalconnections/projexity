//! Compose-stack installs (one-click apps): ship a rendered compose file to
//! the server and drive `docker compose` over SSH. Web services carry the
//! standard routing labels, so the existing Caddy renderer routes them like
//! any other app.

use projexity_core::{DeployError, EventSink};

use crate::docker::DockerServer;
use crate::transport::{shell_quote, NodeChannel};

const STACK_DIR: &str = "/etc/projexity/apps";

impl DockerServer {
    async fn sh_root(&self, cmd: &str) -> Result<crate::transport::ExecOutput, DeployError> {
        // Targets are validated root/passwordless-sudo at bootstrap; compose
        // stacks are managed as root like everything else we install.
        let wrapped = format!("sh -c {}", shell_quote(cmd));
        self.channel
            .exec(&wrapped)
            .await
            .map_err(|e| DeployError::Transport(e.to_string()))
    }

    /// Install or update a compose stack. Idempotent: `compose up -d`
    /// converges.
    pub async fn install_stack(
        &self,
        slug: &str,
        compose_yaml: &str,
        events: &EventSink,
    ) -> Result<(), DeployError> {
        let dir = format!("{STACK_DIR}/{slug}");

        events.step_started("upload");
        let mkdir = self
            .sh_root(&format!("mkdir -p {}", shell_quote(&dir)))
            .await?;
        if !mkdir.success() {
            return Err(DeployError::Provider(format!(
                "couldn't create stack dir: {}",
                mkdir.stderr.trim()
            )));
        }
        let write = self
            .channel
            .exec_with_stdin(
                &format!(
                    "cat > {}",
                    shell_quote(&format!("{dir}/docker-compose.yml"))
                ),
                compose_yaml.as_bytes(),
            )
            .await
            .map_err(|e| DeployError::Transport(e.to_string()))?;
        if !write.success() {
            return Err(DeployError::Provider(format!(
                "couldn't write compose file: {}",
                write.stderr.trim()
            )));
        }
        events.step_completed("upload");

        events.step_started("up");
        events.progress("pulling images and starting services (first install can take a while)");
        let up = self
            .sh_root(&format!(
                "cd {} && docker compose up -d --quiet-pull 2>&1",
                shell_quote(&dir)
            ))
            .await?;
        for line in up
            .stdout
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            if !line.trim().is_empty() {
                events.progress(line.to_string());
            }
        }
        if !up.success() {
            let tail: String = up
                .stdout
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ");
            return Err(DeployError::Provider(format!("compose up failed: {tail}")));
        }
        events.step_completed("up");

        // Route the newly labeled containers.
        events.step_started("route");
        let (docker, _guard) = self.docker().await?;
        self.sync_caddy(&docker, None).await?;
        events.step_completed("route");
        Ok(())
    }

    /// Tear a stack down. Containers and networks go; named volumes stay
    /// unless `remove_volumes` (someone's data deserves an explicit choice).
    pub async fn uninstall_stack(
        &self,
        slug: &str,
        remove_volumes: bool,
    ) -> Result<(), DeployError> {
        let dir = format!("{STACK_DIR}/{slug}");
        let flags = if remove_volumes { "-v" } else { "" };
        let down = self
            .sh_root(&format!(
                "cd {} 2>/dev/null && docker compose down {} 2>&1 || true",
                shell_quote(&dir),
                flags
            ))
            .await?;
        tracing::debug!(stdout = %down.stdout, "compose down");
        let _ = self
            .sh_root(&format!("rm -rf {}", shell_quote(&dir)))
            .await?;
        let (docker, _guard) = self.docker().await?;
        self.sync_caddy(&docker, None).await?;
        Ok(())
    }
}
