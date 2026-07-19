//! Build-plan detection: Dockerfile first, Nixpacks fallback.

use std::path::Path;

/// How an app gets built. The detected plan is always printed to the build
/// log so "why did it think my app is PHP" is self-serviceable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildPlan {
    /// Use the Dockerfile at this path (relative to the project root dir).
    Dockerfile { path: String },
    /// No Dockerfile found: Nixpacks will generate one into the context.
    /// (Implemented in M3 via the `nixpacks` crate.)
    Nixpacks,
}

/// Detect the plan for a checked-out workdir. `dockerfile_override` comes
/// from project settings.
pub fn detect(workdir: &Path, dockerfile_override: Option<&str>) -> BuildPlan {
    if let Some(path) = dockerfile_override {
        return BuildPlan::Dockerfile {
            path: path.to_string(),
        };
    }
    if workdir.join("Dockerfile").is_file() {
        return BuildPlan::Dockerfile {
            path: "Dockerfile".to_string(),
        };
    }
    BuildPlan::Nixpacks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let dir = std::env::temp_dir();
        assert_eq!(
            detect(&dir, Some("docker/Dockerfile.prod")),
            BuildPlan::Dockerfile {
                path: "docker/Dockerfile.prod".into()
            }
        );
    }

    #[test]
    fn falls_back_to_nixpacks() {
        let dir = tempdir_without_dockerfile();
        assert_eq!(detect(&dir, None), BuildPlan::Nixpacks);
    }

    fn tempdir_without_dockerfile() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("pjx-plan-test-empty");
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("Dockerfile"));
        dir
    }
}
