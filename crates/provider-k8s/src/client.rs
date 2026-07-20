//! Building a kube client from a stored kubeconfig, and validating a cluster
//! at onboarding time.

use serde::{Deserialize, Serialize};

/// Provider-specific config for a `k8s_cluster` target (stored as JSONB; the
/// kubeconfig is encrypted upstream, like all credentials).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sConfig {
    /// Encrypted kubeconfig YAML.
    pub kubeconfig_enc: String,
    /// Namespace apps are deployed into (created if absent).
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// IngressClass name (e.g. "traefik", "nginx"). Empty = cluster default.
    #[serde(default)]
    pub ingress_class: String,
    /// cert-manager ClusterIssuer to annotate Ingresses with (empty = none;
    /// user's ingress/TLS setup handles certs).
    #[serde(default)]
    pub cluster_issuer: String,
    /// Base domain for generated hostnames: `<slug>.<domain_base>`. Empty
    /// falls back to sslip against the ingress IP (filled at validate time).
    #[serde(default)]
    pub domain_base: String,
    /// Facts from the last validation, for display.
    #[serde(default)]
    pub info: Option<serde_json::Value>,
}

fn default_namespace() -> String {
    "projexity".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub version: String,
    pub platform: String,
    pub node_count: usize,
    pub ingress_classes: Vec<String>,
    /// Whether the ServiceAccount can create the resources we need.
    pub can_deploy: bool,
    pub warnings: Vec<String>,
}

/// Build a client from a raw kubeconfig YAML string.
pub async fn connect(kubeconfig_yaml: &str, namespace: &str) -> anyhow::Result<kube::Client> {
    let kc = kube::config::Kubeconfig::from_yaml(kubeconfig_yaml)
        .map_err(|e| anyhow::anyhow!("couldn't parse the kubeconfig: {e}"))?;
    // Reject exec-plugin auth up front with a clear message — we can't run
    // `aws`/`gcloud` binaries inside the control plane.
    if kc
        .auth_infos
        .iter()
        .any(|ai| ai.auth_info.as_ref().is_some_and(|a| a.exec.is_some()))
    {
        anyhow::bail!(
            "this kubeconfig uses an exec auth plugin (common on EKS/GKE), which we can't run. \
             Create a long-lived ServiceAccount token and use a token-based kubeconfig instead."
        );
    }
    let options = kube::config::KubeConfigOptions {
        context: kc.current_context.clone(),
        ..Default::default()
    };
    let mut config = kube::Config::from_custom_kubeconfig(kc, &options)
        .await
        .map_err(|e| anyhow::anyhow!("invalid kubeconfig: {e}"))?;
    config.default_namespace = namespace.to_string();
    kube::Client::try_from(config).map_err(|e| anyhow::anyhow!("couldn't build a client: {e}"))
}

/// Connect and gather facts + capability checks. `Err` means we couldn't
/// reach the cluster (message is user-friendly).
pub async fn validate(kubeconfig_yaml: &str, namespace: &str) -> anyhow::Result<ClusterInfo> {
    use k8s_openapi::api::core::v1::Node;
    use k8s_openapi::api::networking::v1::IngressClass;
    use kube::api::{Api, ListParams};

    let client = connect(kubeconfig_yaml, namespace).await?;

    let version = client
        .apiserver_version()
        .await
        .map_err(|e| anyhow::anyhow!("couldn't reach the cluster API: {e}"))?;

    let mut warnings = Vec::new();

    let nodes: Api<Node> = Api::all(client.clone());
    let node_count = nodes
        .list(&ListParams::default())
        .await
        .map(|l| l.items.len())
        .unwrap_or(0);

    let ic: Api<IngressClass> = Api::all(client.clone());
    let ingress_classes: Vec<String> = ic
        .list(&ListParams::default())
        .await
        .map(|l| {
            l.items
                .into_iter()
                .filter_map(|c| c.metadata.name)
                .collect()
        })
        .unwrap_or_default();
    if ingress_classes.is_empty() {
        warnings.push(
            "No IngressClass found — install an ingress controller (ingress-nginx, Traefik) \
             so apps get external URLs."
                .to_string(),
        );
    }

    let can_deploy = can_create_deployments(&client, namespace).await;
    if !can_deploy {
        warnings.push(
            "This kubeconfig may lack permission to create Deployments in the target namespace."
                .to_string(),
        );
    }

    Ok(ClusterInfo {
        version: format!("{}.{}", version.major, version.minor),
        platform: version.platform.clone(),
        node_count,
        ingress_classes,
        can_deploy,
        warnings,
    })
}

/// SelfSubjectAccessReview for creating Deployments in the namespace.
async fn can_create_deployments(client: &kube::Client, namespace: &str) -> bool {
    use k8s_openapi::api::authorization::v1::{
        ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
    };
    use kube::api::{Api, PostParams};

    let review = SelfSubjectAccessReview {
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(ResourceAttributes {
                namespace: Some(namespace.to_string()),
                verb: Some("create".to_string()),
                group: Some("apps".to_string()),
                resource: Some("deployments".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let api: Api<SelfSubjectAccessReview> = Api::all(client.clone());
    match api.create(&PostParams::default(), &review).await {
        Ok(res) => res.status.map(|s| s.allowed).unwrap_or(false),
        // If SSAR itself isn't permitted, don't hard-fail onboarding.
        Err(_) => true,
    }
}
