//! Materializes per-target SSH runtime files (key, known_hosts, control
//! sockets) under the state dir. The database is the source of truth; these
//! files are disposable working copies.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use uuid::Uuid;

pub struct SshRuntime {
    pub key_path: PathBuf,
    pub known_hosts_path: PathBuf,
    pub control_dir: PathBuf,
}

pub fn target_dir(state_dir: &Path, target_id: Uuid) -> PathBuf {
    state_dir.join("targets").join(target_id.to_string())
}

/// Write key material for a target. `known_hosts`: pass the pinned entry from
/// the DB when there is one; pass None on first connect (TOFU will populate
/// the file, and the caller persists it back via [`read_known_hosts`]).
pub fn materialize(
    state_dir: &Path,
    target_id: Uuid,
    private_key: &[u8],
    known_hosts: Option<&str>,
) -> anyhow::Result<SshRuntime> {
    let dir = target_dir(state_dir, target_id);
    fs::create_dir_all(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;

    // Control sockets must live at a SHORT path: unix socket paths cap out
    // around 104 bytes (macOS) and ssh appends `%C` (40 chars) plus a random
    // suffix. /tmp keeps us well under; sockets are ephemeral anyway.
    let control_dir = PathBuf::from(format!(
        "/tmp/pjx-cm/{}",
        &target_id.simple().to_string()[..12]
    ));
    fs::create_dir_all(&control_dir)?;
    fs::set_permissions(&control_dir, fs::Permissions::from_mode(0o700))?;

    let key_path = dir.join("id_ed25519");
    fs::write(&key_path, private_key)?;
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;

    let known_hosts_path = dir.join("known_hosts");
    match known_hosts {
        Some(pinned) => fs::write(&known_hosts_path, pinned)?,
        None => {
            if !known_hosts_path.exists() {
                fs::write(&known_hosts_path, "")?;
            }
        }
    }
    fs::set_permissions(&known_hosts_path, fs::Permissions::from_mode(0o600))?;

    Ok(SshRuntime {
        key_path,
        known_hosts_path,
        control_dir,
    })
}

/// Read back the (possibly TOFU-updated) known_hosts contents for pinning in
/// the DB.
pub fn read_known_hosts(state_dir: &Path, target_id: Uuid) -> Option<String> {
    let content = fs::read_to_string(target_dir(state_dir, target_id).join("known_hosts")).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Remove a target's runtime files (target deletion).
pub fn cleanup(state_dir: &Path, target_id: Uuid) {
    let _ = fs::remove_dir_all(target_dir(state_dir, target_id));
}
