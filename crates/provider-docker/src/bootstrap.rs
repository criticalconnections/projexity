//! Server bootstrap: turn a bare Linux box into a Projexity-ready Docker
//! host. Every step is check-then-act, so re-running ("Repair", version
//! upgrades, crashed halfway) is always safe.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::preflight::{self, ServerFacts};
use crate::transport::{shell_quote, NodeChannel};
use crate::{CADDY_CONTAINER, NETWORK_NAME};

/// Bump when the bootstrap sequence changes; recorded in
/// /etc/projexity/server.json so outdated servers can be flagged.
pub const BOOTSTRAP_VERSION: u32 = 1;

/// Caddy image, pinned. The Caddyfile only opens the admin API (bound to
/// 127.0.0.1 on the host via the port mapping); all routing config arrives
/// later via `POST /load`.
const CADDY_IMAGE: &str = "caddy:2.10";
const CADDYFILE: &str = "{\n\tadmin 0.0.0.0:2019\n}\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepReport {
    pub id: String,
    pub label: String,
    pub status: StepStatus,
    pub detail: String,
}

pub const STEPS: &[(&str, &str)] = &[
    ("preflight", "Checking the server"),
    ("docker", "Installing Docker"),
    ("network", "Creating the app network"),
    ("proxy", "Starting the Caddy proxy (automatic HTTPS)"),
    ("finish", "Finishing up"),
];

pub struct Bootstrap<'a> {
    channel: &'a dyn NodeChannel,
    /// "root" or "sudo" (validated by preflight before any mutation).
    access: String,
    server_id: String,
    progress: mpsc::UnboundedSender<Vec<StepReport>>,
    steps: Vec<StepReport>,
}

impl<'a> Bootstrap<'a> {
    pub fn new(
        channel: &'a dyn NodeChannel,
        server_id: String,
        progress: mpsc::UnboundedSender<Vec<StepReport>>,
    ) -> Self {
        let steps = STEPS
            .iter()
            .map(|(id, label)| StepReport {
                id: (*id).into(),
                label: (*label).into(),
                status: StepStatus::Pending,
                detail: String::new(),
            })
            .collect();
        Self {
            channel,
            access: "root".into(),
            server_id,
            progress,
            steps,
        }
    }

    fn set_step(&mut self, id: &str, status: StepStatus, detail: &str) {
        if let Some(s) = self.steps.iter_mut().find(|s| s.id == id) {
            s.status = status;
            s.detail = detail.to_string();
        }
        let _ = self.progress.send(self.steps.clone());
    }

    /// Run `cmd` through `sh -c`, elevated if the connection user isn't root.
    async fn sh(&self, cmd: &str) -> anyhow::Result<crate::transport::ExecOutput> {
        let wrapped = if self.access == "root" {
            format!("sh -c {}", shell_quote(cmd))
        } else {
            format!("sudo -n sh -c {}", shell_quote(cmd))
        };
        self.channel.exec(&wrapped).await
    }

    async fn sh_ok(&self, cmd: &str, what: &str) -> anyhow::Result<String> {
        let out = self.sh(cmd).await?;
        if !out.success() {
            let err = out.stderr.trim();
            let tail: String = err.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
            anyhow::bail!("{what} failed: {tail}");
        }
        Ok(out.stdout)
    }

    /// Execute the whole sequence. On error the failing step is already
    /// marked `Failed` with detail; the error message is user-facing.
    pub async fn run(mut self) -> anyhow::Result<ServerFacts> {
        // Step 1: preflight (mutates nothing).
        self.set_step("preflight", StepStatus::Running, "");
        let facts = match preflight::run(self.channel).await {
            Ok(f) => f,
            Err(e) => {
                self.set_step("preflight", StepStatus::Failed, &e.to_string());
                return Err(e);
            }
        };
        if facts.has_blocker() {
            let msg = facts
                .issues()
                .iter()
                .find(|i| i.severity == preflight::Severity::Error)
                .map(|i| i.message.clone())
                .unwrap_or_else(|| "preflight found a blocking problem".into());
            self.set_step("preflight", StepStatus::Failed, &msg);
            anyhow::bail!("{msg}");
        }
        self.access = facts.access.clone();
        self.set_step("preflight", StepStatus::Done, "");

        // Step 2: Docker — installed AND the daemon actually answering.
        // (Fresh installs normally start via systemd; minimal images and
        // containers need the fallbacks.)
        let already = facts.docker_version.clone();
        if already.is_none() {
            self.set_step("docker", StepStatus::Running, "downloading get.docker.com");
            if let Err(e) = self
                .sh_ok(
                    "command -v docker >/dev/null 2>&1 || \
                     (curl -fsSL https://get.docker.com | sh)",
                    "Docker install",
                )
                .await
            {
                self.set_step("docker", StepStatus::Failed, &e.to_string());
                return Err(e);
            }
        }
        self.set_step(
            "docker",
            StepStatus::Running,
            "waiting for the Docker daemon",
        );
        let ensure_daemon = r#"
if ! docker info >/dev/null 2>&1; then
  if command -v systemctl >/dev/null 2>&1; then systemctl enable --now docker >/dev/null 2>&1 || true
  elif command -v service >/dev/null 2>&1; then service docker start >/dev/null 2>&1 || true
  fi
  docker info >/dev/null 2>&1 || (dockerd >/var/log/pjx-dockerd.log 2>&1 &)
  i=0; while [ $i -lt 15 ]; do docker info >/dev/null 2>&1 && break; i=$((i+1)); sleep 2; done
fi
docker info >/dev/null 2>&1 && docker --version
"#;
        match self
            .sh_ok(ensure_daemon, "Starting the Docker daemon")
            .await
        {
            Ok(version) => {
                let detail = already.unwrap_or_else(|| version.trim().to_string());
                self.set_step("docker", StepStatus::Done, &detail);
            }
            Err(e) => {
                self.set_step("docker", StepStatus::Failed, &e.to_string());
                return Err(e);
            }
        }

        // Step 3: app network.
        self.set_step("network", StepStatus::Running, "");
        if let Err(e) = self
            .sh_ok(
                &format!(
                    "docker network inspect {NETWORK_NAME} >/dev/null 2>&1 || \
                     docker network create {NETWORK_NAME}"
                ),
                "Network creation",
            )
            .await
        {
            self.set_step("network", StepStatus::Failed, &e.to_string());
            return Err(e);
        }
        self.set_step("network", StepStatus::Done, "");

        // Step 4: Caddy proxy.
        self.set_step("proxy", StepStatus::Running, "");
        if let Err(e) = self.ensure_caddy().await {
            self.set_step("proxy", StepStatus::Failed, &e.to_string());
            return Err(e);
        }
        self.set_step("proxy", StepStatus::Done, "");

        // Step 5: marker file + fresh facts.
        self.set_step("finish", StepStatus::Running, "");
        let marker = serde_json::json!({
            "server_id": self.server_id,
            "bootstrap_version": BOOTSTRAP_VERSION,
        });
        if let Err(e) = self
            .sh_ok(
                &format!(
                    "mkdir -p /etc/projexity && printf '%s\\n' {} > /etc/projexity/server.json",
                    shell_quote(&marker.to_string())
                ),
                "Writing the server marker",
            )
            .await
        {
            self.set_step("finish", StepStatus::Failed, &e.to_string());
            return Err(e);
        }
        let final_facts = preflight::run(self.channel).await.unwrap_or(facts);
        self.set_step("finish", StepStatus::Done, "");
        Ok(final_facts)
    }

    async fn ensure_caddy(&mut self) -> anyhow::Result<()> {
        // Config file first (idempotent overwrite).
        self.sh_ok(
            &format!(
                "mkdir -p /etc/projexity/caddy && printf '%s' {} > /etc/projexity/caddy/Caddyfile",
                shell_quote(CADDYFILE)
            ),
            "Writing the proxy config",
        )
        .await?;

        let state = self
            .sh(&format!(
                "docker inspect -f '{{{{.State.Running}}}}' {CADDY_CONTAINER} 2>/dev/null"
            ))
            .await?;
        match state.stdout.trim() {
            "true" => return Ok(()),
            "false" => {
                self.sh_ok(
                    &format!("docker start {CADDY_CONTAINER}"),
                    "Starting the proxy",
                )
                .await?;
                return Ok(());
            }
            _ => {}
        }

        self.set_step("proxy", StepStatus::Running, "pulling caddy image");
        self.sh_ok(
            &format!(
                "docker run -d --name {CADDY_CONTAINER} \
                 --restart unless-stopped \
                 --network {NETWORK_NAME} \
                 --label projexity.managed=true \
                 -p 80:80 -p 443:443 -p 443:443/udp \
                 -p 127.0.0.1:2019:2019 \
                 -v pjx-caddy-data:/data -v pjx-caddy-config:/config \
                 -v /etc/projexity/caddy/Caddyfile:/etc/caddy/Caddyfile:ro \
                 {CADDY_IMAGE}"
            ),
            "Starting the proxy",
        )
        .await?;
        Ok(())
    }
}
