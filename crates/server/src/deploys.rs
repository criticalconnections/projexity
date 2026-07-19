//! Shared deploy-starter: builds the immutable release snapshot for a project
//! and enqueues the deployment job. Used by the API (`POST .../deploy`) and
//! the GitHub push webhook alike.

use projexity_core::ReleaseId;
use projexity_db::{deployments, env_vars, projects};

use crate::release::{EncEnvPair, ReleaseSnapshot, RepoSpec};
use crate::state::AppState;

/// Returns `Ok(None)` when another deployment for this project is already in
/// flight (the partial unique index makes double-starts impossible).
pub async fn start_deploy(
    state: &AppState,
    p: &projects::Project,
    kind: &str,
) -> anyhow::Result<Option<deployments::Deployment>> {
    let release_id = ReleaseId::new();
    let (image, repo, locally_built) = match (&p.image, &p.repo_owner, &p.repo_name) {
        (Some(image), _, _) => (image.clone(), None, false),
        (None, Some(owner), Some(name)) => (
            format!(
                "pjx/{}:{}",
                p.slug,
                projexity_provider_docker::docker::release_short(&release_id.to_string())
            ),
            Some(RepoSpec {
                owner: owner.clone(),
                name: name.clone(),
                branch: p.branch.clone(),
                dockerfile_path: None,
            }),
            true,
        ),
        _ => anyhow::bail!("project has no image or repository configured"),
    };

    let env_rows = env_vars::list(&state.pool, p.id).await?;
    let env: Vec<EncEnvPair> = env_rows
        .into_iter()
        .filter(|r| !r.is_build_time)
        .map(|r| {
            Ok(EncEnvPair {
                key: r.key,
                value_enc: String::from_utf8(r.value_ciphertext)?,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    let domains = projects::domains(&state.pool, p.id).await?;

    let snapshot = ReleaseSnapshot {
        release_id,
        app_slug: p.slug.clone(),
        image,
        container_port: p.container_port as u16,
        domains,
        env,
        repo,
        locally_built,
    };

    let Some(deployment) =
        deployments::create(&state.pool, p.id, kind, &serde_json::to_value(&snapshot)?).await?
    else {
        return Ok(None);
    };

    projexity_db::jobs::enqueue(
        &state.pool,
        "run_deployment",
        serde_json::json!({ "deployment_id": deployment.id }),
        None,
    )
    .await?;

    Ok(Some(deployment))
}
