//! The release snapshot stored on every deployment row.
//!
//! Env values are stored encrypted (master key) — the plaintext exists only
//! in memory at deploy time. Replaying a snapshot reproduces the release
//! exactly, including its env as of that deploy: that's what makes rollback
//! trustworthy.

use projexity_core::{
    AppRef, DeployPolicy, DomainSpec, EnvPair, HealthSpec, ImageRef, PortProtocol, PortSpec,
    ProjectId, ReleaseId, ReleaseSpec, ResourceSpec, SealedEnv,
};
use serde::{Deserialize, Serialize};

use crate::secrets::MasterKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncEnvPair {
    pub key: String,
    pub value_enc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSpec {
    pub owner: String,
    pub name: String,
    pub branch: String,
    #[serde(default)]
    pub dockerfile_path: Option<String>,
}

impl RepoSpec {
    pub fn clone_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSnapshot {
    pub release_id: ReleaseId,
    pub app_slug: String,
    pub image: String,
    pub container_port: u16,
    pub domains: Vec<String>,
    pub env: Vec<EncEnvPair>,
    /// Set on git-based deploys: build this repo into `image` first.
    /// Rollbacks strip it — the old image is reused, never rebuilt.
    #[serde(default)]
    pub repo: Option<RepoSpec>,
    /// The image only exists on the target's daemon (built there); never pull.
    #[serde(default)]
    pub locally_built: bool,
}

impl ReleaseSnapshot {
    /// Decrypt env and build the in-memory spec the provider consumes.
    pub fn to_spec(
        &self,
        project_id: ProjectId,
        master_key: &MasterKey,
    ) -> anyhow::Result<ReleaseSpec> {
        let mut pairs = Vec::with_capacity(self.env.len());
        for e in &self.env {
            let value = String::from_utf8(master_key.decrypt(&e.value_enc)?)?;
            pairs.push(EnvPair {
                key: e.key.clone(),
                value,
            });
        }
        Ok(ReleaseSpec {
            app: AppRef {
                project_id,
                slug: self.app_slug.clone(),
            },
            release_id: self.release_id,
            image: ImageRef {
                name: self.image.clone(),
                digest: None,
                pull_policy: if self.locally_built {
                    projexity_core::PullPolicy::Never
                } else {
                    projexity_core::PullPolicy::Always
                },
            },
            env: SealedEnv::new(pairs),
            ports: vec![PortSpec {
                container_port: self.container_port,
                protocol: PortProtocol::Http,
            }],
            health: HealthSpec::None,
            resources: ResourceSpec::default(),
            replicas: 1,
            domains: self
                .domains
                .iter()
                .map(|d| DomainSpec {
                    hostname: d.clone(),
                    is_generated: d.ends_with(".sslip.io"),
                })
                .collect(),
            deploy_policy: DeployPolicy::default(),
        })
    }
}

/// The free generated domain for a project on a given server host: sslip.io
/// resolves `<anything>.<ip-with-dashes>.sslip.io` to the embedded IP, so
/// apps get a working hostname before the user configures any DNS.
pub fn generated_domain(slug: &str, host: &str) -> String {
    let ip = match host {
        "localhost" => Some("127-0-0-1".to_string()),
        h => h
            .parse::<std::net::Ipv4Addr>()
            .ok()
            .map(|ip| ip.to_string().replace('.', "-")),
    };
    match ip {
        Some(dashes) => format!("{slug}.{dashes}.sslip.io"),
        // Hostname targets: the user points a wildcard at the host; until
        // then this at least produces a deterministic name.
        None => format!("{slug}.{host}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sslip_for_ips() {
        assert_eq!(
            generated_domain("myapp", "203.0.113.7"),
            "myapp.203-0-113-7.sslip.io"
        );
        assert_eq!(
            generated_domain("myapp", "localhost"),
            "myapp.127-0-0-1.sslip.io"
        );
        assert_eq!(
            generated_domain("myapp", "server.example.com"),
            "myapp.server.example.com"
        );
    }
}
