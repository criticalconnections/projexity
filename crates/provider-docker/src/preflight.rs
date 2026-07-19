//! Fact-gathering preflight: mutate nothing, learn everything, and turn what
//! we learn into specific, actionable messages. This is where onboarding
//! support tickets go to die.

use serde::{Deserialize, Serialize};

use crate::transport::{classify_ssh_error, NodeChannel};

/// One POSIX-sh script, one round trip, KEY=VALUE lines out.
const FACTS_SCRIPT: &str = r#"
echo "PJX_OS=$(uname -s)"
echo "PJX_ARCH=$(uname -m)"
if [ -r /etc/os-release ]; then . /etc/os-release; echo "PJX_DISTRO=$PRETTY_NAME"; fi
if command -v docker >/dev/null 2>&1; then echo "PJX_DOCKER=$(docker --version 2>/dev/null)"; fi
LISTEN="$( (ss -ltn 2>/dev/null || netstat -ltn 2>/dev/null) | awk '{print $4}' )"
echo "$LISTEN" | grep -qE '[:.]80$'  && echo "PJX_PORT80=busy"  || echo "PJX_PORT80=free"
echo "$LISTEN" | grep -qE '[:.]443$' && echo "PJX_PORT443=busy" || echo "PJX_PORT443=free"
echo "PJX_DISK=$(df -k / 2>/dev/null | awk 'NR==2{printf "%.0f", $4*1024}')"
echo "PJX_MEM=$(awk '/MemTotal/{printf "%.0f", $2*1024}' /proc/meminfo 2>/dev/null)"
if [ "$(id -u)" = "0" ]; then echo "PJX_ACCESS=root"
elif sudo -n true 2>/dev/null; then echo "PJX_ACCESS=sudo"
else echo "PJX_ACCESS=none"; fi
if docker inspect -f '{{.State.Running}}' pjx-caddy 2>/dev/null | grep -q true; then echo "PJX_CADDY=running"; fi
if [ -r /etc/projexity/server.json ]; then echo "PJX_MARKER=$(cat /etc/projexity/server.json | tr -d '\n')"; fi
"#;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerFacts {
    pub os: String,
    pub arch: String,
    pub distro: Option<String>,
    pub docker_version: Option<String>,
    pub port_80_free: bool,
    pub port_443_free: bool,
    pub disk_free_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    /// "root" | "sudo" | "none"
    pub access: String,
    /// Our proxy is already up (repair / re-run case).
    pub caddy_running: bool,
    /// Contents of /etc/projexity/server.json if this server was already
    /// bootstrapped (possibly by a different Projexity instance).
    pub marker: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Blocks bootstrap.
    Error,
    /// Bootstrap can proceed; the user should know.
    Warning,
    /// FYI ("Docker isn't installed — we'll install it").
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

impl ServerFacts {
    fn parse(stdout: &str) -> Self {
        let mut f = ServerFacts {
            port_80_free: true,
            port_443_free: true,
            access: "none".into(),
            ..Default::default()
        };
        for line in stdout.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "PJX_OS" => f.os = value.to_string(),
                "PJX_ARCH" => f.arch = value.to_string(),
                "PJX_DISTRO" if !value.is_empty() => f.distro = Some(value.to_string()),
                "PJX_DOCKER" if !value.is_empty() => f.docker_version = Some(value.to_string()),
                "PJX_PORT80" => f.port_80_free = value == "free",
                "PJX_PORT443" => f.port_443_free = value == "free",
                "PJX_DISK" => f.disk_free_bytes = value.parse().ok(),
                "PJX_MEM" => f.memory_total_bytes = value.parse().ok(),
                "PJX_ACCESS" => f.access = value.to_string(),
                "PJX_CADDY" => f.caddy_running = value == "running",
                "PJX_MARKER" if !value.is_empty() => f.marker = Some(value.to_string()),
                _ => {}
            }
        }
        f
    }

    /// Actionable findings, ordered errors-first.
    pub fn issues(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);

        if self.os != "Linux" {
            out.push(Issue {
                severity: Severity::Error,
                message: format!(
                    "This looks like a {} machine — Projexity targets must run Linux.",
                    self.os
                ),
            });
        }
        if self.access == "none" {
            out.push(Issue {
                severity: Severity::Error,
                message: "This user has neither root nor passwordless sudo. Connect as root, or \
                          grant passwordless sudo (`echo \"$USER ALL=(ALL) NOPASSWD:ALL\" | sudo \
                          tee /etc/sudoers.d/projexity`)."
                    .into(),
            });
        }
        // Ports held by our own proxy are fine (repair / reconnect case).
        if !self.caddy_running {
            if !self.port_80_free {
                out.push(Issue {
                    severity: Severity::Warning,
                    message: "Port 80 is in use — probably an existing web server (nginx/apache). \
                              Stop it before deploying, or HTTPS certificates will fail to issue."
                        .into(),
                });
            }
            if !self.port_443_free {
                out.push(Issue {
                    severity: Severity::Warning,
                    message: "Port 443 is in use — another service is already serving HTTPS on \
                              this machine."
                        .into(),
                });
            }
        }
        if let Some(disk) = self.disk_free_bytes {
            if disk < 5 * 1024 * 1024 * 1024 {
                out.push(Issue {
                    severity: Severity::Warning,
                    message: format!(
                        "Only {:.1} GiB of disk free — builds and images need room; 10+ GiB \
                         recommended.",
                        gib(disk)
                    ),
                });
            }
        }
        if let Some(mem) = self.memory_total_bytes {
            if mem < 900 * 1024 * 1024 {
                out.push(Issue {
                    severity: Severity::Warning,
                    message: format!(
                        "{:.1} GiB of RAM is tight — small apps will run, but builds may \
                         struggle. 1+ GiB recommended.",
                        gib(mem)
                    ),
                });
            }
        }
        if self.docker_version.is_none() {
            out.push(Issue {
                severity: Severity::Info,
                message: "Docker isn't installed yet — we'll install it during setup.".into(),
            });
        }
        if self.marker.is_some() && !self.caddy_running {
            out.push(Issue {
                severity: Severity::Info,
                message: "This server was set up by Projexity before — setup will repair it."
                    .into(),
            });
        }
        out.sort_by_key(|i| match i.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        });
        out
    }

    pub fn has_blocker(&self) -> bool {
        self.issues().iter().any(|i| i.severity == Severity::Error)
    }
}

/// Run the preflight. `Err` means we couldn't even talk to the server (the
/// message is already user-friendly); `Ok` carries facts + findings.
pub async fn run(channel: &dyn NodeChannel) -> anyhow::Result<ServerFacts> {
    let out = channel.exec(FACTS_SCRIPT).await?;
    if !out.success() && out.stdout.trim().is_empty() {
        anyhow::bail!("{}", classify_ssh_error(&out.stderr));
    }
    Ok(ServerFacts::parse(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "PJX_OS=Linux\nPJX_ARCH=x86_64\nPJX_DISTRO=Ubuntu 24.04.2 LTS\n\
        PJX_PORT80=free\nPJX_PORT443=busy\nPJX_DISK=52000000000\nPJX_MEM=2000000000\nPJX_ACCESS=root\n";

    #[test]
    fn parses_facts() {
        let f = ServerFacts::parse(SAMPLE);
        assert_eq!(f.os, "Linux");
        assert_eq!(f.distro.as_deref(), Some("Ubuntu 24.04.2 LTS"));
        assert!(f.port_80_free);
        assert!(!f.port_443_free);
        assert_eq!(f.access, "root");
        assert!(f.docker_version.is_none());
    }

    #[test]
    fn flags_no_access_as_blocker() {
        let f = ServerFacts::parse("PJX_OS=Linux\nPJX_ACCESS=none\n");
        assert!(f.has_blocker());
    }

    #[test]
    fn busy_port_is_warning_not_blocker() {
        let f = ServerFacts::parse("PJX_OS=Linux\nPJX_ACCESS=root\nPJX_PORT80=busy\n");
        assert!(!f.has_blocker());
        assert!(f
            .issues()
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("Port 80")));
    }

    #[test]
    fn caddy_running_suppresses_port_warnings() {
        let f = ServerFacts::parse(
            "PJX_OS=Linux\nPJX_ACCESS=root\nPJX_PORT80=busy\nPJX_PORT443=busy\nPJX_CADDY=running\n",
        );
        assert!(!f.issues().iter().any(|i| i.message.contains("Port")));
    }
}
