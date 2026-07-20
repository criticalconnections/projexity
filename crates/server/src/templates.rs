//! One-click app templates: YAML files (templates/*.yaml) declaring a
//! docker-compose stack, its user-facing env schema, generated secrets, and
//! which services get domains.
//!
//! Rendering = interpolate `{{KEY}}` / `{{DOMAIN_<service>}}` through the
//! compose tree, then inject what every stack needs: restart policies, the
//! shared `projexity` network + routing labels on web services, and
//! deterministic container names Caddy can dial.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvField {
    pub key: String,
    /// Auto-generate instead of asking the user: "hex32" | "hex16" |
    /// "base64_32".
    #[serde(default)]
    pub generate: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebService {
    pub service: String,
    pub port: u16,
    /// Suffix for multi-web templates ("" = primary, gets the bare slug
    /// domain; "api" -> <slug>-api.<base>).
    #[serde(default)]
    pub domain_suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub website: String,
    #[serde(default)]
    pub env: Vec<EnvField>,
    pub web: Vec<WebService>,
    pub compose: serde_yaml::Value,
}

/// Catalog entry for the API (generated secrets are internal — the UI only
/// asks for user-facing fields).
#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub website: String,
    pub env: Vec<EnvField>,
}

impl Template {
    pub fn catalog_entry(&self) -> CatalogEntry {
        CatalogEntry {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            category: self.category.clone(),
            icon: self.icon.clone(),
            website: self.website.clone(),
            env: self
                .env
                .iter()
                .filter(|e| e.generate.is_none())
                .cloned()
                .collect(),
        }
    }
}

pub fn load_all(dir: &Path) -> anyhow::Result<Vec<Template>> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        tracing::warn!(?dir, "templates directory missing");
        return Ok(out);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|s| serde_yaml::from_str::<Template>(&s).map_err(Into::into))
        {
            Ok(t) => out.push(t),
            Err(e) => tracing::error!(?path, ?e, "failed to load template"),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Fill generated secrets + defaults, validate required user inputs.
pub fn resolve_env(
    template: &Template,
    user_values: &BTreeMap<String, String>,
) -> anyhow::Result<BTreeMap<String, String>> {
    use rand_core::RngCore;
    let mut out = BTreeMap::new();
    for field in &template.env {
        let value = if let Some(kind) = &field.generate {
            let mut bytes = vec![0u8; if kind.contains("16") { 16 } else { 32 }];
            rand_core::OsRng.fill_bytes(&mut bytes);
            if kind.starts_with("base64") {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            } else {
                bytes.iter().map(|b| format!("{b:02x}")).collect()
            }
        } else if let Some(v) = user_values.get(&field.key) {
            v.clone()
        } else if let Some(d) = &field.default {
            d.clone()
        } else if field.required {
            anyhow::bail!("missing required setting: {}", field.key);
        } else {
            continue;
        };
        out.insert(field.key.clone(), value);
    }
    Ok(out)
}

/// Domain per web service: primary gets `<slug>.<base>`, suffixed services
/// get `<slug>-<suffix>.<base>`.
pub fn assign_domains(template: &Template, slug: &str, host: &str) -> BTreeMap<String, String> {
    template
        .web
        .iter()
        .map(|w| {
            let name = if w.domain_suffix.is_empty() {
                slug.to_string()
            } else {
                format!("{slug}-{}", w.domain_suffix)
            };
            (
                w.service.clone(),
                crate::release::generated_domain(&name, host),
            )
        })
        .collect()
}

/// Render the final compose file text.
pub fn render_compose(
    template: &Template,
    slug: &str,
    env: &BTreeMap<String, String>,
    domains: &BTreeMap<String, String>,
) -> anyhow::Result<String> {
    let mut compose = template.compose.clone();

    // 1. Interpolate {{KEY}} and {{DOMAIN_service}} in every string.
    let mut vars: BTreeMap<String, String> = env.clone();
    for (service, domain) in domains {
        vars.insert(format!("DOMAIN_{service}"), domain.clone());
    }
    interpolate(&mut compose, &vars)?;

    // 2. Structural injection.
    let root = compose
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("template compose must be a mapping"))?;

    // networks: projexity (external) — web services join it for Caddy.
    let networks = root
        .entry("networks".into())
        .or_insert(serde_yaml::Value::Mapping(Default::default()));
    if let Some(m) = networks.as_mapping_mut() {
        m.insert(
            "projexity".into(),
            serde_yaml::from_str("{ external: true }")?,
        );
    }

    let services = root
        .get_mut("services")
        .and_then(|s| s.as_mapping_mut())
        .ok_or_else(|| anyhow::anyhow!("template compose has no services"))?;

    for (svc_name, svc) in services.iter_mut() {
        let name = svc_name.as_str().unwrap_or_default().to_string();
        let Some(svc) = svc.as_mapping_mut() else {
            continue;
        };
        // Everything restarts with the box.
        svc.entry("restart".into())
            .or_insert("unless-stopped".into());

        if let Some(web) = template.web.iter().find(|w| w.service == name) {
            let container = format!("pjx-{slug}-{name}");
            let domain = domains.get(&name).cloned().unwrap_or_default();
            svc.insert("container_name".into(), container.into());

            // Routing labels — the same convention app deploys use, so the
            // existing Caddy renderer picks these up unchanged.
            let labels = svc
                .entry("labels".into())
                .or_insert(serde_yaml::Value::Mapping(Default::default()));
            if let Some(l) = labels.as_mapping_mut() {
                l.insert("projexity.managed".into(), "true".into());
                l.insert("projexity.app".into(), format!("{slug}-{name}").into());
                l.insert("projexity.port".into(), web.port.to_string().into());
                l.insert("projexity.domains".into(), domain.into());
            }

            // default (stack-internal) + projexity (proxy-reachable).
            svc.insert(
                "networks".into(),
                serde_yaml::from_str("[default, projexity]")?,
            );
        }
    }

    Ok(serde_yaml::to_string(&compose)?)
}

fn interpolate(
    value: &mut serde_yaml::Value,
    vars: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    match value {
        serde_yaml::Value::String(s) => {
            if s.contains("{{") {
                let mut out = s.clone();
                for (k, v) in vars {
                    out = out.replace(&format!("{{{{{k}}}}}"), v);
                }
                if out.contains("{{") {
                    anyhow::bail!("unresolved template variable in: {out}");
                }
                *s = out;
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                interpolate(item, vars)?;
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map.iter_mut() {
                interpolate(v, vars)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Template {
        serde_yaml::from_str(
            r#"
id: kuma
name: Uptime Kuma
description: Self-hosted uptime monitoring
category: monitoring
icon: "📈"
website: https://uptime.kuma.pet
env:
  - key: SECRET
    generate: hex32
  - key: TZ
    label: Timezone
    default: UTC
web:
  - service: kuma
    port: 3001
compose:
  services:
    kuma:
      image: louislam/uptime-kuma:1
      environment:
        TZ: "{{TZ}}"
        APP_SECRET: "{{SECRET}}"
        PUBLIC_URL: "https://{{DOMAIN_kuma}}"
      volumes:
        - kuma_data:/app/data
  volumes:
    kuma_data: {}
"#,
        )
        .unwrap()
    }

    #[test]
    fn env_resolution_generates_and_defaults() {
        let t = sample();
        let env = resolve_env(&t, &BTreeMap::new()).unwrap();
        assert_eq!(env.get("TZ").unwrap(), "UTC");
        assert_eq!(env.get("SECRET").unwrap().len(), 64); // hex32
    }

    #[test]
    fn render_injects_routing() {
        let t = sample();
        let env = resolve_env(&t, &BTreeMap::new()).unwrap();
        let domains = assign_domains(&t, "status", "203.0.113.7");
        let yaml = render_compose(&t, "status", &env, &domains).unwrap();
        assert!(yaml.contains("container_name: pjx-status-kuma"));
        assert!(yaml.contains("projexity.app: status-kuma"));
        assert!(yaml.contains("projexity.port: '3001'"));
        assert!(yaml.contains("status.203-0-113-7.sslip.io"));
        assert!(yaml.contains("external: true"));
        assert!(yaml.contains("restart: unless-stopped"));
        assert!(!yaml.contains("{{"));
        // catalog hides generated secrets
        assert_eq!(t.catalog_entry().env.len(), 1);
    }

    #[test]
    fn every_repo_template_loads_and_renders() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
        let templates = load_all(&dir).unwrap();
        assert!(
            templates.len() >= 20,
            "expected the template library, found {}",
            templates.len()
        );
        for t in &templates {
            // Fill required user fields with a dummy value.
            let user: BTreeMap<String, String> = t
                .env
                .iter()
                .filter(|e| e.required && e.generate.is_none())
                .map(|e| (e.key.clone(), "dummy-value".to_string()))
                .collect();
            let env = resolve_env(t, &user)
                .unwrap_or_else(|e| panic!("{}: env resolution failed: {e}", t.id));
            let domains = assign_domains(t, "testapp", "203.0.113.7");
            let yaml = render_compose(t, "testapp", &env, &domains)
                .unwrap_or_else(|e| panic!("{}: render failed: {e}", t.id));
            assert!(!yaml.contains("{{"), "{}: unresolved variables", t.id);
            assert!(!t.web.is_empty(), "{}: no web services", t.id);
            for w in &t.web {
                assert!(
                    yaml.contains(&format!("pjx-testapp-{}", w.service)),
                    "{}: web service {} not routed",
                    t.id,
                    w.service
                );
            }
        }
    }

    #[test]
    fn unresolved_variable_fails_loudly() {
        let mut t = sample();
        t.env.retain(|e| e.key != "SECRET");
        let env = resolve_env(&t, &BTreeMap::new()).unwrap();
        let domains = assign_domains(&t, "s", "1.2.3.4");
        assert!(render_compose(&t, "s", &env, &domains).is_err());
    }
}
