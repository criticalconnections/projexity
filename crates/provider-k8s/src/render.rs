//! Render a `ReleaseSpec` into Kubernetes objects. One function per kind;
//! `provider` applies them with server-side apply.

use std::collections::BTreeMap;

use k8s_openapi::api::apps::v1::{
    Deployment, DeploymentSpec, DeploymentStrategy, RollingUpdateDeployment,
};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, HTTPGetAction, Namespace, PodSpec, PodTemplateSpec, Probe,
    ResourceRequirements, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::api::networking::v1::{
    HTTPIngressPath, HTTPIngressRuleValue, Ingress, IngressBackend, IngressRule,
    IngressServiceBackend, IngressSpec, IngressTLS, ServiceBackendPort,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use projexity_core::{HealthSpec, ReleaseSpec};

use crate::{LABEL_APP, LABEL_MANAGED, LABEL_RELEASE};

/// K8s resource name for an app (DNS-1123: lowercase, already slug-safe).
pub fn resource_name(slug: &str) -> String {
    format!("pjx-{slug}")
}

fn labels(spec: &ReleaseSpec) -> BTreeMap<String, String> {
    BTreeMap::from([
        (LABEL_MANAGED.to_string(), "projexity".to_string()),
        (LABEL_APP.to_string(), spec.app.slug.clone()),
    ])
}

fn selector_labels(slug: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(LABEL_APP.to_string(), slug.to_string())])
}

pub fn namespace(name: &str) -> Namespace {
    Namespace {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            labels: Some(BTreeMap::from([(
                LABEL_MANAGED.to_string(),
                "projexity".to_string(),
            )])),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn deployment(spec: &ReleaseSpec, ns: &str) -> Deployment {
    let name = resource_name(&spec.app.slug);
    let port = spec.ports.first().map(|p| p.container_port).unwrap_or(80) as i32;

    let env: Vec<EnvVar> = spec
        .env
        .pairs()
        .iter()
        .map(|p| EnvVar {
            name: p.key.clone(),
            value: Some(p.value.clone()),
            ..Default::default()
        })
        .collect();

    let mut limits = BTreeMap::new();
    let mut requests = BTreeMap::new();
    if let Some(cpu) = spec.resources.cpu_millicores {
        limits.insert(
            "cpu".to_string(),
            k8s_openapi::apimachinery::pkg::api::resource::Quantity(format!("{cpu}m")),
        );
    }
    if let Some(mem) = spec.resources.memory_bytes {
        let q = k8s_openapi::apimachinery::pkg::api::resource::Quantity(mem.to_string());
        limits.insert("memory".to_string(), q.clone());
        requests.insert("memory".to_string(), q);
    }
    let resources = if limits.is_empty() && requests.is_empty() {
        None
    } else {
        Some(ResourceRequirements {
            limits: (!limits.is_empty()).then_some(limits),
            requests: (!requests.is_empty()).then_some(requests),
            ..Default::default()
        })
    };

    let readiness = readiness_probe(&spec.health, port);

    // Release id in a pod annotation forces a rollout even when the image tag
    // is unchanged (e.g. redeploying `:latest`).
    let mut pod_annotations = BTreeMap::new();
    pod_annotations.insert(LABEL_RELEASE.to_string(), spec.release_id.to_string());

    Deployment {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(ns.to_string()),
            labels: Some(labels(spec)),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(spec.replicas.max(1) as i32),
            selector: LabelSelector {
                match_labels: Some(selector_labels(&spec.app.slug)),
                ..Default::default()
            },
            strategy: Some(DeploymentStrategy {
                type_: Some("RollingUpdate".to_string()),
                rolling_update: Some(RollingUpdateDeployment {
                    max_unavailable: Some(IntOrString::Int(0)),
                    max_surge: Some(IntOrString::Int(1)),
                }),
            }),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(selector_labels(&spec.app.slug)),
                    annotations: Some(pod_annotations),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "app".to_string(),
                        image: Some(spec.image.name.clone()),
                        ports: Some(vec![ContainerPort {
                            container_port: port,
                            ..Default::default()
                        }]),
                        env: (!env.is_empty()).then_some(env),
                        resources,
                        readiness_probe: readiness,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn readiness_probe(health: &HealthSpec, default_port: i32) -> Option<Probe> {
    match health {
        HealthSpec::Http {
            path,
            port,
            initial_delay_secs,
            period_secs,
            timeout_secs,
            failure_threshold,
        } => Some(Probe {
            http_get: Some(HTTPGetAction {
                path: Some(path.clone()),
                port: IntOrString::Int(*port as i32),
                ..Default::default()
            }),
            initial_delay_seconds: Some(*initial_delay_secs as i32),
            period_seconds: Some(*period_secs as i32),
            timeout_seconds: Some(*timeout_secs as i32),
            failure_threshold: Some(*failure_threshold as i32),
            ..Default::default()
        }),
        HealthSpec::Tcp {
            port,
            initial_delay_secs,
            period_secs,
            ..
        } => Some(Probe {
            tcp_socket: Some(k8s_openapi::api::core::v1::TCPSocketAction {
                port: IntOrString::Int(*port as i32),
                ..Default::default()
            }),
            initial_delay_seconds: Some(*initial_delay_secs as i32),
            period_seconds: Some(*period_secs as i32),
            ..Default::default()
        }),
        HealthSpec::None => {
            // A minimal TCP readiness on the app port keeps rollouts honest
            // (don't route until the port accepts connections).
            Some(Probe {
                tcp_socket: Some(k8s_openapi::api::core::v1::TCPSocketAction {
                    port: IntOrString::Int(default_port),
                    ..Default::default()
                }),
                initial_delay_seconds: Some(2),
                period_seconds: Some(5),
                ..Default::default()
            })
        }
    }
}

pub fn service(spec: &ReleaseSpec, ns: &str) -> Service {
    let name = resource_name(&spec.app.slug);
    let port = spec.ports.first().map(|p| p.container_port).unwrap_or(80) as i32;
    Service {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(ns.to_string()),
            labels: Some(labels(spec)),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(selector_labels(&spec.app.slug)),
            ports: Some(vec![ServicePort {
                port: 80,
                target_port: Some(IntOrString::Int(port)),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn ingress(
    spec: &ReleaseSpec,
    ns: &str,
    ingress_class: &str,
    cluster_issuer: &str,
) -> Option<Ingress> {
    if spec.domains.is_empty() {
        return None;
    }
    let name = resource_name(&spec.app.slug);

    let backend = IngressBackend {
        service: Some(IngressServiceBackend {
            name: name.clone(),
            port: Some(ServiceBackendPort {
                number: Some(80),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };

    let rules: Vec<IngressRule> = spec
        .domains
        .iter()
        .map(|d| IngressRule {
            host: Some(d.hostname.clone()),
            http: Some(HTTPIngressRuleValue {
                paths: vec![HTTPIngressPath {
                    path: Some("/".to_string()),
                    path_type: "Prefix".to_string(),
                    backend: backend.clone(),
                }],
            }),
        })
        .collect();

    let mut annotations = BTreeMap::new();
    let tls = if cluster_issuer.is_empty() {
        None
    } else {
        annotations.insert(
            "cert-manager.io/cluster-issuer".to_string(),
            cluster_issuer.to_string(),
        );
        Some(vec![IngressTLS {
            hosts: Some(spec.domains.iter().map(|d| d.hostname.clone()).collect()),
            secret_name: Some(format!("{name}-tls")),
        }])
    };

    Some(Ingress {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(ns.to_string()),
            labels: Some(labels(spec)),
            annotations: (!annotations.is_empty()).then_some(annotations),
            ..Default::default()
        },
        spec: Some(IngressSpec {
            ingress_class_name: (!ingress_class.is_empty()).then(|| ingress_class.to_string()),
            rules: Some(rules),
            tls,
            ..Default::default()
        }),
        ..Default::default()
    })
}
