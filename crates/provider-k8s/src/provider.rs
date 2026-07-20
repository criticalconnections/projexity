//! The Kubernetes deploy engine: server-side apply the rendered objects, then
//! watch the Deployment roll out. Semantics mirror the Docker provider — a
//! failed rollout leaves the previous ReplicaSet serving — but Kubernetes
//! does the choreography, so we apply and observe rather than orchestrate.

use std::time::Duration;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Namespace, Pod, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{Api, DeleteParams, ListParams, LogParams, Patch, PatchParams};
use projexity_core::{DeployError, EventSink, ReleaseSpec};

use crate::client::{connect, K8sConfig};
use crate::render;
use crate::{FIELD_MANAGER, LABEL_APP};

pub struct K8sProvider {
    pub client: kube::Client,
    pub config: K8sConfig,
}

impl K8sProvider {
    pub async fn new(kubeconfig_yaml: &str, config: K8sConfig) -> Result<Self, DeployError> {
        let client = connect(kubeconfig_yaml, &config.namespace)
            .await
            .map_err(|e| DeployError::Transport(e.to_string()))?;
        Ok(Self { client, config })
    }

    async fn apply<K>(
        &self,
        api: &Api<K>,
        name: &str,
        obj: &K,
        events: &EventSink,
    ) -> Result<(), DeployError>
    where
        K: kube::Resource
            + Clone
            + serde::Serialize
            + serde::de::DeserializeOwned
            + std::fmt::Debug,
        K::DynamicType: Default,
    {
        let params = PatchParams::apply(FIELD_MANAGER).force();
        api.patch(name, &params, &Patch::Apply(obj))
            .await
            .map_err(|e| DeployError::Provider(format!("apply {name} failed: {e}")))?;
        let _ = events;
        Ok(())
    }

    /// Apply all objects and wait for the rollout. Returns the provider ref
    /// (namespace + deployment name) for status/logs/destroy.
    pub async fn deploy_release(
        &self,
        spec: &ReleaseSpec,
        events: &EventSink,
    ) -> Result<serde_json::Value, DeployError> {
        let ns = &self.config.namespace;

        // Namespace (cluster-scoped).
        events.step_started("namespace");
        let ns_api: Api<Namespace> = Api::all(self.client.clone());
        self.apply(&ns_api, ns, &render::namespace(ns), events)
            .await?;
        events.step_completed("namespace");

        let name = render::resource_name(&spec.app.slug);

        events.step_started("apply");
        let dep_api: Api<Deployment> = Api::namespaced(self.client.clone(), ns);
        self.apply(&dep_api, &name, &render::deployment(spec, ns), events)
            .await?;

        let svc_api: Api<Service> = Api::namespaced(self.client.clone(), ns);
        self.apply(&svc_api, &name, &render::service(spec, ns), events)
            .await?;

        if let Some(ing) = render::ingress(
            spec,
            ns,
            &self.config.ingress_class,
            &self.config.cluster_issuer,
        ) {
            let ing_api: Api<Ingress> = Api::namespaced(self.client.clone(), ns);
            self.apply(&ing_api, &name, &ing, events).await?;
            events.progress(format!(
                "ingress created for {}",
                spec.domains
                    .iter()
                    .map(|d| d.hostname.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        events.step_completed("apply");

        // Watch the rollout.
        events.step_started("rollout");
        self.await_rollout(ns, &name, spec, events).await?;
        events.emit(projexity_core::DeployEvent::TrafficShifted {
            release_id: spec.release_id,
        });
        events.step_completed("rollout");

        Ok(serde_json::json!({ "namespace": ns, "deployment": name }))
    }

    /// Poll the Deployment until the new generation is fully available or the
    /// deadline passes. Surfaces pod failures (ImagePullBackOff /
    /// CrashLoopBackOff) verbatim.
    async fn await_rollout(
        &self,
        ns: &str,
        name: &str,
        spec: &ReleaseSpec,
        events: &EventSink,
    ) -> Result<(), DeployError> {
        let dep_api: Api<Deployment> = Api::namespaced(self.client.clone(), ns);
        let pod_api: Api<Pod> = Api::namespaced(self.client.clone(), ns);
        let deadline = spec.deploy_policy.health_gate_timeout_seconds.max(60) as u64;
        let started = tokio::time::Instant::now();
        let want = spec.replicas.max(1) as i32;

        loop {
            if started.elapsed() > Duration::from_secs(deadline) {
                let reason = self.pod_failure_reason(&pod_api, &spec.app.slug).await;
                return Err(DeployError::HealthGateFailed(format!(
                    "rollout didn't become ready within {deadline}s{}",
                    reason.map(|r| format!(" — {r}")).unwrap_or_default()
                )));
            }

            let dep = dep_api
                .get(name)
                .await
                .map_err(|e| DeployError::Provider(format!("reading deployment: {e}")))?;
            let status = dep.status.unwrap_or_default();
            let updated = status.updated_replicas.unwrap_or(0);
            let available = status.available_replicas.unwrap_or(0);
            let ready = status.ready_replicas.unwrap_or(0);

            // Fail fast on a stuck pull/crash instead of waiting out the clock.
            if let Some(reason) = self.pod_failure_reason(&pod_api, &spec.app.slug).await {
                return Err(DeployError::HealthGateFailed(reason));
            }

            if updated >= want && available >= want && ready >= want {
                events.emit(projexity_core::DeployEvent::HealthProbe {
                    healthy: true,
                    detail: format!("{ready}/{want} replicas ready"),
                });
                return Ok(());
            }
            events.progress(format!("rolling out: {ready}/{want} ready"));
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    /// Inspect this app's pods for a terminal container state worth reporting.
    async fn pod_failure_reason(&self, pod_api: &Api<Pod>, slug: &str) -> Option<String> {
        let lp = ListParams::default().labels(&format!("{LABEL_APP}={slug}"));
        let pods = pod_api.list(&lp).await.ok()?;
        for pod in pods.items {
            let statuses = pod.status?.container_statuses?;
            for cs in statuses {
                if let Some(waiting) = cs.state.as_ref().and_then(|s| s.waiting.as_ref()) {
                    let reason = waiting.reason.clone().unwrap_or_default();
                    if reason == "ImagePullBackOff"
                        || reason == "ErrImagePull"
                        || reason == "CrashLoopBackOff"
                        || reason == "CreateContainerConfigError"
                    {
                        let msg = waiting.message.clone().unwrap_or(reason.clone());
                        return Some(format!("{reason}: {msg}"));
                    }
                }
            }
        }
        None
    }

    pub async fn app_status(&self, slug: &str) -> Result<serde_json::Value, DeployError> {
        let ns = &self.config.namespace;
        let dep_api: Api<Deployment> = Api::namespaced(self.client.clone(), ns);
        let name = render::resource_name(slug);
        match dep_api.get_opt(&name).await {
            Ok(Some(dep)) => {
                let status = dep.status.unwrap_or_default();
                let ready = status.ready_replicas.unwrap_or(0);
                let want = dep.spec.and_then(|s| s.replicas).unwrap_or(1);
                Ok(serde_json::json!({
                    "health": if ready >= want && want > 0 { "healthy" } else { "unhealthy" },
                    "ready_replicas": ready,
                    "desired_replicas": want,
                }))
            }
            Ok(None) => Ok(serde_json::json!({ "health": "stopped" })),
            Err(e) => Err(DeployError::Provider(e.to_string())),
        }
    }

    /// Stream logs of the app's newest pod.
    pub async fn runtime_logs(
        &self,
        slug: &str,
        tail: i64,
    ) -> Result<impl futures::Stream<Item = String>, DeployError> {
        use futures::{AsyncBufReadExt, StreamExt};
        let ns = &self.config.namespace;
        let pod_api: Api<Pod> = Api::namespaced(self.client.clone(), ns);
        let lp = ListParams::default().labels(&format!("{LABEL_APP}={slug}"));
        let pods = pod_api
            .list(&lp)
            .await
            .map_err(|e| DeployError::Provider(e.to_string()))?;
        let pod = pods
            .items
            .into_iter()
            .max_by_key(|p| {
                p.metadata
                    .creation_timestamp
                    .as_ref()
                    .map(|t| t.0)
                    .unwrap_or_default()
            })
            .ok_or_else(|| DeployError::Provider("no running pods for this app".into()))?;
        let pod_name = pod.metadata.name.unwrap_or_default();
        let params = LogParams {
            follow: true,
            tail_lines: Some(tail),
            ..Default::default()
        };
        let stream = pod_api
            .log_stream(&pod_name, &params)
            .await
            .map_err(|e| DeployError::Provider(e.to_string()))?;
        Ok(stream.lines().filter_map(|l| async move { l.ok() }))
    }

    /// Delete all of an app's resources (by name; labels for the ingress).
    pub async fn destroy_app(&self, slug: &str) -> Result<(), DeployError> {
        let ns = &self.config.namespace;
        let name = render::resource_name(slug);
        let dp = DeleteParams::default();

        let dep_api: Api<Deployment> = Api::namespaced(self.client.clone(), ns);
        let _ = dep_api.delete(&name, &dp).await;
        let svc_api: Api<Service> = Api::namespaced(self.client.clone(), ns);
        let _ = svc_api.delete(&name, &dp).await;
        let ing_api: Api<Ingress> = Api::namespaced(self.client.clone(), ns);
        let _ = ing_api.delete(&name, &dp).await;
        Ok(())
    }
}
