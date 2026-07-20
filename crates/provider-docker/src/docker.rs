//! The real Docker deploy engine: bollard over an SSH-forwarded docker
//! socket, blue/green choreography, Caddy config sync.
//!
//! Every resource carries the managed labels; the Caddy config is rendered
//! purely from live container labels (domains + port), so the proxy state is
//! always derivable from reality — no database in the loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use bollard::models::{ContainerCreateBody, HostConfig, RestartPolicy, RestartPolicyNameEnum};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, InspectContainerOptions,
    ListContainersOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
    StartContainerOptions, StopContainerOptionsBuilder,
};
use bollard::Docker;
use futures::StreamExt;
use projexity_core::{DeployError, EventSink, ReleaseSpec};

use crate::transport::{shell_quote, NodeChannel, SshChannel};
use crate::{CADDY_CONTAINER, NETWORK_NAME};

pub const LABEL_MANAGED: &str = "projexity.managed";
pub const LABEL_APP: &str = "projexity.app";
pub const LABEL_RELEASE: &str = "projexity.release";
pub const LABEL_PORT: &str = "projexity.port";
pub const LABEL_DOMAINS: &str = "projexity.domains";

/// Keeps the ssh socket-forward process alive while a Docker client uses it.
pub struct ForwardGuard {
    child: tokio::process::Child,
}

impl Drop for ForwardGuard {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

pub struct DockerServer {
    pub channel: SshChannel,
}

impl DockerServer {
    /// Forward the remote docker socket to a local unix socket and connect.
    pub async fn docker(&self) -> Result<(Docker, ForwardGuard), DeployError> {
        let sock: PathBuf = self.channel.control_dir.join("d.sock");
        let _ = std::fs::remove_file(&sock);

        let mut cmd = tokio::process::Command::new("ssh");
        cmd.arg("-F")
            .arg("none")
            .arg("-i")
            .arg(&self.channel.key_path)
            .arg("-p")
            .arg(self.channel.port.to_string())
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                self.channel.known_hosts_path.display()
            ))
            .arg("-o")
            .arg("StrictHostKeyChecking=accept-new")
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-o")
            .arg("StreamLocalBindUnlink=yes")
            // Deliberately NOT multiplexed (-S none): a killed forward client
            // leaves its forward registered in a shared ControlMaster, which
            // then refuses the next identical forward. A dedicated connection
            // dies clean.
            .arg("-S")
            .arg("none")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg("-N")
            .arg("-L")
            .arg(format!("{}:/var/run/docker.sock", sock.display()))
            .arg(format!("{}@{}", self.channel.user, self.channel.host))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let child = cmd
            .spawn()
            .map_err(|e| DeployError::Transport(format!("failed to start ssh forward: {e}")))?;

        // Wait for the local socket to appear.
        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !sock.exists() {
            return Err(DeployError::Transport(
                "SSH port-forward to the Docker socket did not come up".into(),
            ));
        }

        let docker =
            Docker::connect_with_unix(&sock.to_string_lossy(), 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| DeployError::Transport(format!("docker connect failed: {e}")))?;
        Ok((docker, ForwardGuard { child }))
    }

    /// Pull an image, streaming condensed progress into the event sink.
    pub async fn pull_image(
        &self,
        docker: &Docker,
        image: &str,
        events: &EventSink,
    ) -> Result<(), DeployError> {
        let opts = CreateImageOptionsBuilder::default()
            .from_image(image)
            .build();
        let mut stream = docker.create_image(Some(opts), None, None);
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(info) => {
                    if let Some(status) = info.status {
                        // Keep the log readable: drop per-layer byte counters.
                        if status.starts_with("Pulling from")
                            || status.starts_with("Digest")
                            || status.starts_with("Status")
                            || status.contains("Pull complete")
                            || status.contains("Already exists")
                        {
                            let line = match info.id {
                                Some(id) => format!("{status} ({id})"),
                                None => status,
                            };
                            events.progress(line);
                        }
                    }
                }
                Err(e) => {
                    return Err(DeployError::ImageUnavailable(format!(
                        "pull of {image} failed: {e}"
                    )))
                }
            }
        }
        Ok(())
    }

    /// The blue/green deploy. Idempotent: deterministic container names mean
    /// re-running after a crash finishes or no-ops.
    pub async fn deploy_release(
        &self,
        spec: &ReleaseSpec,
        events: &EventSink,
    ) -> Result<String, DeployError> {
        let (docker, _guard) = self.docker().await?;
        let name = container_name(spec);
        let port = spec.ports.first().map(|p| p.container_port).unwrap_or(80);
        let domains: Vec<String> = spec.domains.iter().map(|d| d.hostname.clone()).collect();

        events.step_started("pull");
        match spec.image.pull_policy {
            projexity_core::PullPolicy::Always => {
                events.progress(format!("Pulling {}", spec.image));
                self.pull_image(&docker, &spec.image.name, events).await?;
            }
            projexity_core::PullPolicy::Never => {
                // Locally-built image: it must already be in the daemon's
                // store (the build ran right here).
                docker.inspect_image(&spec.image.name).await.map_err(|_| {
                    DeployError::ImageUnavailable(format!(
                        "image {} is not on the server — it may have been pruned; \
                             deploy again to rebuild it",
                        spec.image
                    ))
                })?;
                events.progress(format!("using locally built image {}", spec.image));
            }
        }
        events.step_completed("pull");

        // Start green (skip if it already exists from a crashed attempt).
        events.step_started("start");
        let existing = docker
            .inspect_container(&name, None::<InspectContainerOptions>)
            .await
            .ok();
        let running = existing
            .as_ref()
            .and_then(|c| c.state.as_ref())
            .and_then(|s| s.running)
            .unwrap_or(false);
        if let Some(_c) = existing {
            if !running {
                docker
                    .remove_container(
                        &name,
                        Some(RemoveContainerOptionsBuilder::default().force(true).build()),
                    )
                    .await
                    .map_err(|e| DeployError::Provider(format!("cleanup of {name}: {e}")))?;
            }
        }
        if !running {
            let mut labels = HashMap::new();
            labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
            labels.insert(LABEL_APP.to_string(), spec.app.slug.clone());
            labels.insert(
                LABEL_RELEASE.to_string(),
                release_short(&spec.release_id.to_string()),
            );
            labels.insert(LABEL_PORT.to_string(), port.to_string());
            labels.insert(LABEL_DOMAINS.to_string(), domains.join(","));

            let env: Vec<String> = spec
                .env
                .pairs()
                .iter()
                .map(|p| format!("{}={}", p.key, p.value))
                .collect();

            let body = ContainerCreateBody {
                image: Some(spec.image.name.clone()),
                env: Some(env),
                labels: Some(labels),
                host_config: Some(HostConfig {
                    network_mode: Some(NETWORK_NAME.to_string()),
                    restart_policy: Some(RestartPolicy {
                        name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                        maximum_retry_count: None,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            };
            docker
                .create_container(
                    Some(CreateContainerOptionsBuilder::default().name(&name).build()),
                    body,
                )
                .await
                .map_err(|e| DeployError::Provider(format!("container create failed: {e}")))?;
            docker
                .start_container(&name, None::<StartContainerOptions>)
                .await
                .map_err(|e| DeployError::Provider(format!("container start failed: {e}")))?;
        }
        events.step_completed("start");

        // Health gate. M2: settle-delay gate — the container must still be
        // running after a short window. (HealthSpec::Http installs a real
        // HEALTHCHECK in M3.)
        events.step_started("health");
        let settle = Duration::from_secs(3);
        tokio::time::sleep(settle).await;
        let state = docker
            .inspect_container(&name, None::<InspectContainerOptions>)
            .await
            .map_err(|e| DeployError::Provider(format!("inspect failed: {e}")))?;
        let alive = state
            .state
            .as_ref()
            .and_then(|s| s.running)
            .unwrap_or(false);
        if !alive {
            let tail = self.container_log_tail(&docker, &name, 100).await;
            // Failed deploys never touched blue — clean up green and bail.
            let _ = docker
                .remove_container(
                    &name,
                    Some(RemoveContainerOptionsBuilder::default().force(true).build()),
                )
                .await;
            return Err(DeployError::HealthGateFailed(format!(
                "container exited during startup. Last log lines:\n{tail}"
            )));
        }
        events.emit(projexity_core::DeployEvent::HealthProbe {
            healthy: true,
            detail: "container is running".into(),
        });
        events.step_completed("health");

        // Traffic cutover: render the full desired config and atomically load.
        events.step_started("cutover");
        self.sync_caddy(&docker, Some((&spec.app.slug, &name)))
            .await?;
        events.emit(projexity_core::DeployEvent::TrafficShifted {
            release_id: spec.release_id,
        });
        events.step_completed("cutover");

        // Verify through the front door (from the server itself, so no
        // external reachability assumptions).
        events.step_started("verify");
        if let Some(domain) = domains.first() {
            let out = self
                .channel
                .exec(&format!(
                    "curl -s -o /dev/null -w '%{{http_code}}' --max-time 10 -H {} http://127.0.0.1/",
                    shell_quote(&format!("Host: {domain}"))
                ))
                .await
                .map_err(|e| DeployError::Transport(e.to_string()))?;
            let code = out.stdout.trim().to_string();
            if code == "502" || code == "503" || code == "000" || code.is_empty() {
                return Err(DeployError::CutoverFailed(format!(
                    "the proxy can't reach the app (HTTP {code}) — is the app listening on port {port}?"
                )));
            }
            events.progress(format!("front door answered HTTP {code} for {domain}"));
        }
        events.step_completed("verify");

        // Retire blue: any same-app container from another release.
        events.step_started("retire");
        let old = self
            .list_managed(&docker)
            .await?
            .into_iter()
            .filter(|c| c.app.as_deref() == Some(spec.app.slug.as_str()) && c.name != name)
            .collect::<Vec<_>>();
        for c in old {
            events.progress(format!("draining {}", c.name));
            let _ = docker
                .stop_container(
                    &c.name,
                    Some(
                        StopContainerOptionsBuilder::default()
                            .t(spec.deploy_policy.drain_seconds as i32)
                            .build(),
                    ),
                )
                .await;
            let _ = docker
                .remove_container(
                    &c.name,
                    Some(RemoveContainerOptionsBuilder::default().force(true).build()),
                )
                .await;
        }
        events.step_completed("retire");

        Ok(name)
    }

    /// Remove every container for an app and drop its routes.
    pub async fn destroy_app(&self, slug: &str) -> Result<(), DeployError> {
        let (docker, _guard) = self.docker().await?;
        for c in self.list_managed(&docker).await? {
            if c.app.as_deref() == Some(slug) {
                let _ = docker
                    .stop_container(
                        &c.name,
                        Some(StopContainerOptionsBuilder::default().t(10).build()),
                    )
                    .await;
                let _ = docker
                    .remove_container(
                        &c.name,
                        Some(RemoveContainerOptionsBuilder::default().force(true).build()),
                    )
                    .await;
            }
        }
        self.sync_caddy(&docker, None).await?;
        Ok(())
    }

    /// Follow runtime logs of the newest container for an app.
    pub async fn runtime_log_stream(
        &self,
        slug: &str,
        tail: i64,
    ) -> Result<
        (
            impl futures::Stream<Item = String> + Send + 'static,
            ForwardGuard,
        ),
        DeployError,
    > {
        let (docker, guard) = self.docker().await?;
        let containers = self.list_managed(&docker).await?;
        let target = containers
            .into_iter()
            .filter(|c| c.app.as_deref() == Some(slug))
            .max_by(|a, b| a.created.cmp(&b.created))
            .ok_or_else(|| DeployError::Provider("no running container for this app".into()))?;

        let opts = LogsOptionsBuilder::default()
            .stdout(true)
            .stderr(true)
            .follow(true)
            .tail(&tail.to_string())
            .timestamps(false)
            .build();
        let stream = docker
            .logs(&target.name, Some(opts))
            .filter_map(|item| async move {
                match item {
                    Ok(chunk) => Some(String::from_utf8_lossy(&chunk.into_bytes()).into_owned()),
                    Err(_) => None,
                }
            });
        Ok((stream, guard))
    }

    /// Build an image on this server's daemon from a tar context, streaming
    /// build output into the event sink. Uses the classic builder (works on
    /// every daemon; BuildKit is a later upgrade).
    pub async fn build_image(
        &self,
        context_tar: Vec<u8>,
        tag: &str,
        events: &EventSink,
    ) -> Result<(), projexity_core::BuildError> {
        use projexity_core::BuildError;
        let (docker, _guard) = self
            .docker()
            .await
            .map_err(|e| BuildError::Transport(e.to_string()))?;
        let opts = bollard::query_parameters::BuildImageOptionsBuilder::default()
            .t(tag)
            .rm(true)
            .build();
        let mut stream =
            docker.build_image(opts, None, Some(bollard::body_full(context_tar.into())));
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(info) => {
                    if let Some(err) = info.error {
                        return Err(BuildError::Build(err));
                    }
                    if let Some(line) = info.stream {
                        for l in line.lines() {
                            if !l.trim().is_empty() {
                                events.progress(l.to_string());
                            }
                        }
                    }
                    if let Some(status) = info.status {
                        if status.starts_with("Pulling from")
                            || status.starts_with("Status")
                            || status.contains("Pull complete")
                        {
                            events.progress(status);
                        }
                    }
                }
                Err(e) => return Err(BuildError::Build(e.to_string())),
            }
        }
        Ok(())
    }

    async fn container_log_tail(&self, docker: &Docker, name: &str, lines: i64) -> String {
        let opts = LogsOptionsBuilder::default()
            .stdout(true)
            .stderr(true)
            .tail(&lines.to_string())
            .build();
        let mut out = String::new();
        let mut stream = docker.logs(name, Some(opts));
        while let Some(Ok(chunk)) = stream.next().await {
            out.push_str(&String::from_utf8_lossy(&chunk.into_bytes()));
        }
        out
    }

    async fn list_managed(&self, docker: &Docker) -> Result<Vec<ManagedContainer>, DeployError> {
        let mut filters = HashMap::new();
        filters.insert("label".to_string(), vec![format!("{LABEL_MANAGED}=true")]);
        let opts = ListContainersOptionsBuilder::default()
            .all(true)
            .filters(&filters)
            .build();
        let list = docker
            .list_containers(Some(opts))
            .await
            .map_err(|e| DeployError::Provider(format!("container list failed: {e}")))?;
        Ok(list
            .into_iter()
            .filter_map(|c| {
                let labels = c.labels.unwrap_or_default();
                let name = c
                    .names
                    .unwrap_or_default()
                    .first()?
                    .trim_start_matches('/')
                    .to_string();
                // The proxy itself is managed but not an app.
                if name == CADDY_CONTAINER {
                    return None;
                }
                Some(ManagedContainer {
                    name,
                    app: labels.get(LABEL_APP).cloned(),
                    port: labels.get(LABEL_PORT).and_then(|p| p.parse().ok()),
                    domains: labels
                        .get(LABEL_DOMAINS)
                        .map(|d| {
                            d.split(',')
                                .filter(|s| !s.is_empty())
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default(),
                    running: c
                        .state
                        .map(|s| s == bollard::models::ContainerSummaryStateEnum::RUNNING)
                        .unwrap_or(false),
                    created: c.created.unwrap_or(0),
                })
            })
            .collect())
    }

    /// Render the full desired Caddy config from live container labels and
    /// POST it to the admin API (from the server itself — the admin port is
    /// bound to the host's loopback).
    ///
    /// `prefer`: (app_slug, container_name) — during a deploy, route this
    /// app's traffic to the named (new) container even though the old one is
    /// still up. That's the atomic cutover.
    pub async fn sync_caddy(
        &self,
        docker: &Docker,
        prefer: Option<(&str, &str)>,
    ) -> Result<(), DeployError> {
        let containers = self.list_managed(&docker.clone()).await?;
        let mut routes = Vec::new();
        let mut seen_apps = std::collections::HashSet::new();

        // Deterministic order: newest container per app wins, preferred app
        // pinned to the named container.
        let mut by_recency = containers.clone();
        by_recency.sort_by_key(|c| std::cmp::Reverse(c.created));

        for c in &by_recency {
            let Some(app) = &c.app else { continue };
            if !c.running || c.domains.is_empty() {
                continue;
            }
            if let Some((slug, name)) = prefer {
                if app == slug && c.name != name {
                    continue;
                }
            }
            if !seen_apps.insert(app.clone()) {
                continue;
            }
            let port = c.port.unwrap_or(80);
            routes.push(serde_json::json!({
                "match": [{"host": c.domains}],
                "handle": [{
                    "handler": "reverse_proxy",
                    "upstreams": [{"dial": format!("{}:{}", c.name, port)}]
                }],
                "terminal": true
            }));
        }

        // Playground domains (sslip.io names embedding loopback/private IPs)
        // can never get public ACME certificates — issue them from Caddy's
        // internal CA instead so apps needing a secure context still work.
        let internal_subjects: Vec<String> = by_recency
            .iter()
            .flat_map(|c| c.domains.iter())
            .filter(|d| needs_internal_tls(d))
            .cloned()
            .collect();
        let mut apps = serde_json::json!({
            "http": {
                "servers": {
                    "pjx": {
                        "listen": [":80", ":443"],
                        // Keep plain HTTP serving the app (no forced
                        // redirect) so apps work before certificates are
                        // issued; TLS still auto-provisions per domain.
                        "automatic_https": {"disable_redirects": true},
                        "routes": routes
                    }
                }
            }
        });
        if !internal_subjects.is_empty() {
            apps["tls"] = serde_json::json!({
                "automation": {
                    "policies": [{
                        "subjects": internal_subjects,
                        "issuers": [{"module": "internal"}]
                    }]
                }
            });
        }
        let config = serde_json::json!({
            "admin": {"listen": "0.0.0.0:2019"},
            "apps": apps
        });

        let out = self
            .channel
            .exec_with_stdin(
                "curl -sf -X POST -H 'Content-Type: application/json' --data-binary @- \
                 http://127.0.0.1:2019/load",
                config.to_string().as_bytes(),
            )
            .await
            .map_err(|e| DeployError::Transport(e.to_string()))?;
        if !out.success() {
            return Err(DeployError::CutoverFailed(format!(
                "Caddy rejected the config: {}",
                out.stderr.trim()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ManagedContainer {
    name: String,
    app: Option<String>,
    port: Option<u16>,
    domains: Vec<String>,
    running: bool,
    created: i64,
}

/// Short unique suffix for container names. Uses the TAIL of the UUID:
/// release ids are UUIDv7, whose leading chars are a coarse timestamp that
/// collides for releases created within the same ~65s window — the tail is
/// the random part.
pub fn release_short(release_id: &str) -> String {
    let s = release_id.replace('-', "");
    s[s.len().saturating_sub(12)..].to_string()
}

/// True for hostnames whose certificates must come from the local CA:
/// sslip.io names embedding a loopback or private IPv4 (dash notation).
pub fn needs_internal_tls(domain: &str) -> bool {
    let Some(embedded) = domain.strip_suffix(".sslip.io") else {
        return false;
    };
    let Some(ip_part) = embedded.rsplit('.').next() else {
        return false;
    };
    let Ok(ip) = ip_part.replace('-', ".").parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    ip.is_loopback() || ip.is_private()
}

pub fn container_name(spec: &ReleaseSpec) -> String {
    format!(
        "pjx-{}-{}",
        spec.app.slug,
        release_short(&spec.release_id.to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_tls_for_local_sslip_only() {
        assert!(needs_internal_tls("memos.127-0-0-1.sslip.io"));
        assert!(needs_internal_tls("app.192-168-1-20.sslip.io"));
        assert!(needs_internal_tls("app.10-0-0-5.sslip.io"));
        assert!(!needs_internal_tls("app.203-0-113-7.sslip.io"));
        assert!(!needs_internal_tls("app.example.com"));
    }

    #[test]
    fn release_short_uses_random_tail() {
        // Two v7 uuids minted in the same timestamp window share a prefix but
        // must yield different shorts.
        let a = "019f7bc8-a23c-7e63-abdc-485a2f040c25";
        let b = "019f7bc8-a23c-7e63-abdc-99ee12345678";
        assert_ne!(release_short(a), release_short(b));
        assert_eq!(release_short(a).len(), 12);
    }
}
