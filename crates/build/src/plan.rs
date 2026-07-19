//! Build-plan detection: Dockerfile first, then built-in generators for
//! common stacks. The detected plan is always printed to the build log so
//! "why did it think my app is PHP" is self-serviceable.
//!
//! Generators are deliberately simple, readable Dockerfiles — the kind a user
//! could copy into their repo and own. (A Nixpacks/Railpack planner can slot
//! in behind the same interface later.)

use std::path::Path;

use projexity_core::BuildError;

/// How an app gets built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildPlan {
    /// Use the Dockerfile at this path (relative to the project root dir).
    Dockerfile { path: String },
    /// No Dockerfile: we generate one into the build context.
    Generated {
        stack: &'static str,
        dockerfile: String,
    },
}

impl BuildPlan {
    /// One-line summary for the build log.
    pub fn summary(&self) -> String {
        match self {
            BuildPlan::Dockerfile { path } => format!("using Dockerfile at {path}"),
            BuildPlan::Generated { stack, .. } => {
                format!("no Dockerfile found — generated one for a {stack} app")
            }
        }
    }
}

/// Detect the plan for a checked-out workdir. `dockerfile_override` comes
/// from project settings. `port` is the container port the app should listen
/// on (generators wire it through).
pub fn detect(
    workdir: &Path,
    dockerfile_override: Option<&str>,
    port: u16,
) -> Result<BuildPlan, BuildError> {
    if let Some(path) = dockerfile_override {
        if !workdir.join(path).is_file() {
            return Err(BuildError::PlanDetection(format!(
                "configured Dockerfile '{path}' doesn't exist in the repository"
            )));
        }
        return Ok(BuildPlan::Dockerfile {
            path: path.to_string(),
        });
    }
    if workdir.join("Dockerfile").is_file() {
        return Ok(BuildPlan::Dockerfile {
            path: "Dockerfile".to_string(),
        });
    }
    if workdir.join("package.json").is_file() {
        return Ok(BuildPlan::Generated {
            stack: "Node.js",
            dockerfile: node_dockerfile(workdir, port),
        });
    }
    if workdir.join("requirements.txt").is_file() {
        return Ok(BuildPlan::Generated {
            stack: "Python",
            dockerfile: python_dockerfile(port),
        });
    }
    if workdir.join("index.html").is_file() {
        return Ok(BuildPlan::Generated {
            stack: "static site",
            dockerfile: static_dockerfile(),
        });
    }
    Err(BuildError::PlanDetection(
        "couldn't detect how to build this repository — add a Dockerfile (or a package.json, \
         requirements.txt, or index.html for auto-detection)"
            .to_string(),
    ))
}

fn node_dockerfile(workdir: &Path, port: u16) -> String {
    let has_lockfile = workdir.join("package-lock.json").is_file();
    let install = if has_lockfile {
        "npm ci"
    } else {
        "npm install"
    };
    // `npm start` is the convention; a missing start script fails loudly in
    // the build log where the user can see exactly why.
    format!(
        "FROM node:22-alpine\n\
         WORKDIR /app\n\
         COPY package*.json ./\n\
         RUN {install} --omit=dev\n\
         COPY . .\n\
         ENV NODE_ENV=production PORT={port}\n\
         EXPOSE {port}\n\
         CMD [\"npm\", \"start\"]\n"
    )
}

fn python_dockerfile(port: u16) -> String {
    format!(
        "FROM python:3.12-slim\n\
         WORKDIR /app\n\
         COPY requirements.txt ./\n\
         RUN pip install --no-cache-dir -r requirements.txt\n\
         COPY . .\n\
         ENV PORT={port}\n\
         EXPOSE {port}\n\
         CMD [\"sh\", \"-c\", \"if [ -f main.py ]; then python main.py; else python app.py; fi\"]\n"
    )
}

fn static_dockerfile() -> String {
    "FROM nginx:alpine\nCOPY . /usr/share/nginx/html\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pjx-plan-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dockerfile_wins() {
        let dir = tmp("dockerfile");
        fs::write(dir.join("Dockerfile"), "FROM scratch").unwrap();
        fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(
            detect(&dir, None, 3000).unwrap(),
            BuildPlan::Dockerfile {
                path: "Dockerfile".into()
            }
        );
    }

    #[test]
    fn node_detected() {
        let dir = tmp("node");
        fs::write(dir.join("package.json"), "{}").unwrap();
        match detect(&dir, None, 3000).unwrap() {
            BuildPlan::Generated { stack, dockerfile } => {
                assert_eq!(stack, "Node.js");
                assert!(dockerfile.contains("npm install"));
                assert!(dockerfile.contains("PORT=3000"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn static_site_detected() {
        let dir = tmp("static");
        fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        match detect(&dir, None, 80).unwrap() {
            BuildPlan::Generated { stack, .. } => assert_eq!(stack, "static site"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_stack_is_actionable() {
        let dir = tmp("unknown");
        let err = detect(&dir, None, 80).unwrap_err();
        assert!(err.to_string().contains("add a Dockerfile"));
    }

    #[test]
    fn missing_override_errors() {
        let dir = tmp("override");
        assert!(detect(&dir, Some("docker/Dockerfile"), 80).is_err());
    }
}
