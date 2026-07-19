use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// New time-ordered (UUIDv7) id.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(v: Uuid) -> Self {
                Self(v)
            }
        }

        impl From<$name> for Uuid {
            fn from(v: $name) -> Uuid {
                v.0
            }
        }
    };
}

define_id!(UserId);
define_id!(TargetId);
define_id!(ProjectId);
define_id!(BuildId);
define_id!(DeploymentId);
define_id!(
    /// Identifies one immutable release. Appears in container/pod names
    /// (`pjx-<app-slug>-<release-short>`), so it must be stable and unique.
    ReleaseId
);

/// Stable reference to a deployable app on a target, used by providers to
/// label and later find every resource they create.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppRef {
    pub project_id: ProjectId,
    /// DNS-safe slug used in container names, network aliases, and labels.
    pub slug: String,
}

impl AppRef {
    /// Deterministic prefix for every resource belonging to this app.
    pub fn resource_prefix(&self) -> String {
        format!("pjx-{}", self.slug)
    }
}
