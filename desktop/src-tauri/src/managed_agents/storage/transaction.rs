use std::path::Path;

use super::atomic_write_json_restricted;

// ── Two-store byte-level rollback ─────────────────────────────────────────
//
// Shared by `commands::teams::adopt::apply` (catalog adoption) and
// `managed_agents::teams` (adopted-team deletion). Identical rollback policy
// in both paths (I5 / I6).

/// Raw pre-write snapshot of a JSON store file.
///
/// `None` means the file did not exist at snapshot time; restoring `None`
/// removes the file (with `NotFound` treated as success — desired state
/// already reached).
pub(crate) type StoreSnapshot = Option<Vec<u8>>;

/// Snapshot the raw bytes of `path`, or `None` if the file is absent.
pub(crate) fn snapshot_store(path: &Path) -> Result<StoreSnapshot, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("failed to snapshot {}: {e}", path.display())),
    }
}

/// Restore `path` from a [`StoreSnapshot`].
///
/// `NotFound` when restoring an absent snap is treated as success — the
/// desired state is already reached (I5).
pub(crate) fn restore_store(path: &Path, snap: StoreSnapshot) -> Result<(), String> {
    match snap {
        Some(bytes) => atomic_write_json_restricted(path, &bytes),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!(
                "failed to remove {} during restore: {e}",
                path.display()
            )),
        },
    }
}

/// Write both stores via the supplied callbacks, rolling back both from
/// caller-supplied snapshots on any failure.
///
/// Both restores are attempted independently, so a restore failure in one
/// store does not prevent the other; errors from both are aggregated (I5).
pub(crate) fn commit_stores_with_snapshots(
    personas_path: &Path,
    teams_path: &Path,
    personas_snap: StoreSnapshot,
    teams_snap: StoreSnapshot,
    write_personas: impl FnOnce() -> Result<(), String>,
    write_teams: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if let Err(error) = write_personas().and_then(|()| write_teams()) {
        let personas_err = restore_store(personas_path, personas_snap).err();
        let teams_err = restore_store(teams_path, teams_snap).err();
        let restore_errors: Vec<&str> = [personas_err.as_deref(), teams_err.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        if !restore_errors.is_empty() {
            return Err(format!(
                "{error} (and the local stores could not be restored: {})",
                restore_errors.join("; ")
            ));
        }
        return Err(error);
    }
    Ok(())
}
