use serde::{Deserialize, Serialize};

/// Lifecycle of a build (git ref -> container image).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildStatus {
    Queued,
    Cloning,
    Building,
    Succeeded,
    Failed,
    Canceled,
    /// A newer queued build for the same project replaced this one before it
    /// started (push 5 commits, build once).
    Superseded,
}

impl BuildStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            BuildStatus::Succeeded
                | BuildStatus::Failed
                | BuildStatus::Canceled
                | BuildStatus::Superseded
        )
    }

    /// Valid transitions; anything else is a bug and must be rejected at the
    /// persistence layer.
    pub fn can_transition_to(self, next: BuildStatus) -> bool {
        use BuildStatus::*;
        matches!(
            (self, next),
            (Queued, Cloning)
                | (Queued, Canceled)
                | (Queued, Superseded)
                | (Cloning, Building)
                | (Cloning, Failed)
                | (Cloning, Canceled)
                | (Cloning, Superseded)
                | (Building, Succeeded)
                | (Building, Failed)
                | (Building, Canceled)
                | (Building, Superseded)
        )
    }
}

/// Lifecycle of a deployment (image -> serving traffic).
///
/// A rollback is a *new* deployment row pointing at an older release spec,
/// not a state on the failed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Pending,
    Deploying,
    /// Traffic cut over; verifying through the front door.
    Verifying,
    Running,
    /// Replaced by a newer running deployment.
    Superseded,
    /// Stopped by user action.
    Stopped,
    Failed,
}

impl DeploymentStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            DeploymentStatus::Superseded | DeploymentStatus::Stopped | DeploymentStatus::Failed
        )
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            DeploymentStatus::Pending | DeploymentStatus::Deploying | DeploymentStatus::Verifying
        )
    }

    pub fn can_transition_to(self, next: DeploymentStatus) -> bool {
        use DeploymentStatus::*;
        matches!(
            (self, next),
            (Pending, Deploying)
                | (Pending, Failed)
                | (Pending, Superseded)
                | (Deploying, Verifying)
                | (Deploying, Failed)
                | (Verifying, Running)
                | (Verifying, Failed)
                | (Running, Superseded)
                | (Running, Stopped)
                | (Running, Failed)
        )
    }
}

/// Lifecycle of a connected target (server or cluster).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    Pending,
    Bootstrapping,
    Ready,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_happy_path() {
        assert!(BuildStatus::Queued.can_transition_to(BuildStatus::Cloning));
        assert!(BuildStatus::Cloning.can_transition_to(BuildStatus::Building));
        assert!(BuildStatus::Building.can_transition_to(BuildStatus::Succeeded));
    }

    #[test]
    fn build_no_resurrection() {
        for terminal in [
            BuildStatus::Succeeded,
            BuildStatus::Failed,
            BuildStatus::Canceled,
            BuildStatus::Superseded,
        ] {
            assert!(terminal.is_terminal());
            for next in [
                BuildStatus::Queued,
                BuildStatus::Cloning,
                BuildStatus::Building,
            ] {
                assert!(!terminal.can_transition_to(next));
            }
        }
    }

    #[test]
    fn deployment_happy_path() {
        assert!(DeploymentStatus::Pending.can_transition_to(DeploymentStatus::Deploying));
        assert!(DeploymentStatus::Deploying.can_transition_to(DeploymentStatus::Verifying));
        assert!(DeploymentStatus::Verifying.can_transition_to(DeploymentStatus::Running));
        assert!(DeploymentStatus::Running.can_transition_to(DeploymentStatus::Superseded));
    }

    #[test]
    fn deployment_cannot_skip_verification() {
        assert!(!DeploymentStatus::Deploying.can_transition_to(DeploymentStatus::Running));
        assert!(!DeploymentStatus::Pending.can_transition_to(DeploymentStatus::Running));
    }
}
