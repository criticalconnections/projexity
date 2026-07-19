use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::DeploymentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStreamKind {
    Build,
    Runtime,
    Deploy,
}

/// One normalized log line, from any source (BuildKit JSONL, docker logs,
/// kube pod logs). `seq` is monotonically increasing per deployment and is
/// the SSE `Last-Event-ID` resume cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub deployment_id: DeploymentId,
    pub seq: u64,
    pub stream: LogStreamKind,
    pub ts: DateTime<Utc>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LogOpts {
    pub follow: bool,
    /// Resume after this sequence number (exclusive).
    pub since_seq: Option<u64>,
    pub tail_lines: Option<u32>,
}

/// Mask secret values in a log line, GitHub-Actions style. Best effort: it
/// catches the common case of a framework dumping its config on boot.
pub fn redact_secrets<'a>(text: &str, secret_values: impl Iterator<Item = &'a str>) -> String {
    let mut out = text.to_string();
    for value in secret_values {
        if !value.is_empty() && out.contains(value) {
            out = out.replace(value, "***");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_values() {
        let secrets = ["s3cretpassword", "another-secret"];
        let line = "connecting with password=s3cretpassword to db";
        assert_eq!(
            redact_secrets(line, secrets.iter().copied()),
            "connecting with password=*** to db"
        );
    }

    #[test]
    fn leaves_clean_lines_alone() {
        let secrets = ["s3cretpassword"];
        let line = "listening on port 3000";
        assert_eq!(redact_secrets(line, secrets.iter().copied()), line);
    }
}
