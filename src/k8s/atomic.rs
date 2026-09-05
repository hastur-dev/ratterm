//! Writing a file so a reader never sees a half-written one.
//!
//! Both the settings file and the cached remote kubeconfigs are written this
//! way, matching how the SSH host store writes `~/.ratterm/ssh_hosts.toml`.

use std::io::Write;
use std::path::Path;

use super::{K8sError, Result};

/// Writes `contents` to `path` through a temporary file in the same directory.
///
/// The rename is what makes the write atomic: a reader sees either the old
/// file or the new one, never a half-written one. The temporary file is
/// removed if the write fails, so a failed save leaves nothing behind.
///
/// # Errors
/// [`K8sError::Storage`] if the directory cannot be created or the file cannot
/// be written.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            K8sError::Storage(format!("{} could not be created: {e}", parent.display()))
        })?;
    }

    let temp_path = path.with_extension("tmp");
    let write = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(contents)?;
        file.flush()?;
        file.sync_all()
    })();

    if let Err(e) = write {
        let _ = std::fs::remove_file(&temp_path);
        return Err(K8sError::Storage(format!(
            "{} could not be written: {e}",
            path.display()
        )));
    }

    // Windows will not rename onto an existing file, so the old one goes
    // first. The temporary file is kept until the rename succeeds, so the
    // content is never lost.
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(K8sError::Storage(format!(
            "{} could not be replaced: {e}",
            path.display()
        )));
    }

    // Best effort: the settings are not secret, but a cached kubeconfig can
    // carry a token, and the file names the clusters this machine talks to.
    let _ = crate::secrets::vault::restrict_permissions_public(path);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_creates_the_file_and_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("file.bin");

        write_atomic(&path, b"first").expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), b"first");
    }

    #[test]
    fn write_atomic_replaces_existing_content_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.bin");

        write_atomic(&path, b"first").expect("first write");
        write_atomic(&path, b"second-and-longer").expect("second write");
        assert_eq!(std::fs::read(&path).expect("read"), b"second-and-longer");
    }

    #[test]
    fn write_atomic_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.bin");
        write_atomic(&path, b"content").expect("write");

        let entries: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["file.bin".to_string()]);
    }

    #[test]
    fn writing_an_empty_file_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");
        write_atomic(&path, b"").expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), Vec::<u8>::new());
    }

    #[test]
    fn writing_where_a_directory_already_stands_reports_a_storage_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("occupied");
        std::fs::create_dir(&path).expect("mkdir");

        match write_atomic(&path, b"content") {
            Err(K8sError::Storage(message)) => assert!(message.contains("occupied"), "{message}"),
            other => panic!("expected a storage error, got {other:?}"),
        }
    }
}
