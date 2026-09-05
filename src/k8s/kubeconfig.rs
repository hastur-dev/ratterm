//! Finding and reading kubeconfig files.
//!
//! `kubectl` is never invoked. The files are parsed with `kube`'s own
//! `Kubeconfig` type, which is also what the client is built from later, so
//! the list the user picks a context from and the connection that is opened
//! cannot disagree about what a context means.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use kube::config::Kubeconfig;

use super::{K8sError, Result};

/// Largest kubeconfig this module will read.
///
/// A kubeconfig is a few kilobytes of YAML even with dozens of clusters. The
/// cap stops a mistyped `KUBECONFIG` pointing at a large file from being read
/// into memory.
const MAX_KUBECONFIG_BYTES: u64 = 4 * 1024 * 1024;

/// Returns the kubeconfig files to read, in the order kubectl would read them.
///
/// `KUBECONFIG` wins when it is set and non-empty, and may list several files
/// separated by the platform's path separator. Otherwise `~/.kube/config` is
/// used.
#[must_use]
pub fn kubeconfig_paths() -> Vec<PathBuf> {
    paths_from(std::env::var_os("KUBECONFIG").as_deref(), dirs::home_dir())
}

/// Resolves the kubeconfig search path from an explicit environment value.
///
/// Split from [`kubeconfig_paths`] so the precedence rules can be tested
/// without mutating the process environment, which is shared by every test in
/// the binary.
#[must_use]
pub fn paths_from(env_value: Option<&OsStr>, home: Option<PathBuf>) -> Vec<PathBuf> {
    if let Some(value) = env_value {
        let listed: Vec<PathBuf> = std::env::split_paths(value)
            .filter(|p| !p.as_os_str().is_empty())
            .collect();
        if !listed.is_empty() {
            return listed;
        }
    }
    home.map(|h| vec![h.join(".kube").join("config")])
        .unwrap_or_default()
}

/// Reads and merges the kubeconfig files this machine is configured to use.
///
/// # Errors
/// [`K8sError::NoKubeconfig`] if none of the files exist, or
/// [`K8sError::MalformedKubeconfig`] if one of them cannot be parsed.
pub fn load_kubeconfig() -> Result<Kubeconfig> {
    load_kubeconfig_from(&kubeconfig_paths())
}

/// Reads and merges an explicit list of kubeconfig files.
///
/// Files that do not exist are skipped, matching kubectl; if none of them
/// exist the result is [`K8sError::NoKubeconfig`].
///
/// # Errors
/// See [`load_kubeconfig`].
pub fn load_kubeconfig_from(paths: &[PathBuf]) -> Result<Kubeconfig> {
    let mut merged: Option<Kubeconfig> = None;

    for path in paths {
        if !path.is_file() {
            continue;
        }
        let parsed = read_one(path)?;
        merged = Some(match merged {
            None => parsed,
            Some(existing) => {
                existing
                    .merge(parsed)
                    .map_err(|e| K8sError::MalformedKubeconfig {
                        path: path.display().to_string(),
                        reason: e.to_string(),
                    })?
            }
        });
    }

    merged.ok_or(K8sError::NoKubeconfig)
}

/// Reads one kubeconfig file, enforcing the size cap.
fn read_one(path: &Path) -> Result<Kubeconfig> {
    let size = std::fs::metadata(path)
        .map_err(|e| K8sError::MalformedKubeconfig {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?
        .len();
    if size > MAX_KUBECONFIG_BYTES {
        return Err(K8sError::MalformedKubeconfig {
            path: path.display().to_string(),
            reason: format!(
                "the file is {size} bytes, larger than the {MAX_KUBECONFIG_BYTES} byte limit"
            ),
        });
    }

    Kubeconfig::read_from(path).map_err(|e| K8sError::MalformedKubeconfig {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::io::Write;

    use super::*;

    fn write_config(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, body).expect("write");
        path
    }

    #[test]
    fn a_missing_kubeconfig_reports_that_there_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(matches!(
            load_kubeconfig_from(&[missing]),
            Err(K8sError::NoKubeconfig)
        ));
    }

    #[test]
    fn an_empty_path_list_reports_that_there_is_none() {
        assert!(matches!(
            load_kubeconfig_from(&[]),
            Err(K8sError::NoKubeconfig)
        ));
    }

    #[test]
    fn a_malformed_kubeconfig_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "config", "clusters: [ this: is: not: yaml\n");

        match load_kubeconfig_from(std::slice::from_ref(&path)) {
            Err(K8sError::MalformedKubeconfig {
                path: reported,
                reason,
            }) => {
                assert_eq!(reported, path.display().to_string());
                assert!(!reason.is_empty());
            }
            other => panic!("expected MalformedKubeconfig, got {other:?}"),
        }
    }

    #[test]
    fn an_oversized_kubeconfig_is_refused_before_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let filler = "#".repeat(1024);
        let mut file = std::fs::File::create(&path).expect("create");
        for _ in 0..(MAX_KUBECONFIG_BYTES / 1024 + 2) {
            writeln!(file, "{filler}").expect("write");
        }
        drop(file);

        match load_kubeconfig_from(&[path]) {
            Err(K8sError::MalformedKubeconfig { reason, .. }) => {
                assert!(reason.contains("limit"), "{reason}");
            }
            other => panic!("expected MalformedKubeconfig, got {other:?}"),
        }
    }

    #[test]
    fn several_kubeconfig_files_are_merged() {
        let dir = tempfile::tempdir().unwrap();
        let first = write_config(
            &dir,
            "first",
            r#"
apiVersion: v1
kind: Config
current-context: one
clusters:
  - name: c1
    cluster: { server: "https://one.invalid:6443" }
contexts:
  - name: one
    context: { cluster: c1, user: u1 }
"#,
        );
        let second = write_config(
            &dir,
            "second",
            r#"
apiVersion: v1
kind: Config
clusters:
  - name: c2
    cluster: { server: "https://two.invalid:6443" }
contexts:
  - name: two
    context: { cluster: c2, user: u2 }
"#,
        );

        let merged = load_kubeconfig_from(&[first, second]).expect("load");
        assert_eq!(merged.contexts.len(), 2);
        assert_eq!(merged.clusters.len(), 2);
        assert_eq!(merged.current_context.as_deref(), Some("one"));
    }

    #[test]
    fn a_missing_file_in_a_list_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let good = write_config(
            &dir,
            "config",
            "apiVersion: v1\nkind: Config\ncontexts: []\nclusters: []\nusers: []\n",
        );
        let missing = dir.path().join("gone");
        assert!(load_kubeconfig_from(&[missing, good]).is_ok());
    }

    #[test]
    fn kubeconfig_env_with_several_paths_is_split() {
        let joined = std::env::join_paths([Path::new("/a/one"), Path::new("/b/two")]).unwrap();
        let paths = paths_from(Some(joined.as_os_str()), Some(PathBuf::from("/home/u")));
        assert_eq!(
            paths,
            vec![PathBuf::from("/a/one"), PathBuf::from("/b/two")]
        );
    }

    #[test]
    fn kubeconfig_env_with_one_path_is_used_alone() {
        let value = OsString::from("/only/config");
        let paths = paths_from(Some(value.as_os_str()), Some(PathBuf::from("/home/u")));
        assert_eq!(paths, vec![PathBuf::from("/only/config")]);
    }

    #[test]
    fn an_empty_kubeconfig_env_falls_back_to_the_home_directory() {
        let value = OsString::new();
        let paths = paths_from(Some(value.as_os_str()), Some(PathBuf::from("/home/u")));
        assert_eq!(paths, vec![PathBuf::from("/home/u/.kube/config")]);
    }

    #[test]
    fn no_kubeconfig_env_uses_the_home_directory() {
        let paths = paths_from(None, Some(PathBuf::from("/home/u")));
        assert_eq!(paths, vec![PathBuf::from("/home/u/.kube/config")]);
    }

    #[test]
    fn with_no_env_and_no_home_there_are_no_paths() {
        assert!(paths_from(None, None).is_empty());
    }

    #[test]
    fn the_default_search_path_is_readable() {
        // Reads the real environment; asserts only that the call is total.
        let paths = kubeconfig_paths();
        assert!(paths.len() <= 64, "unexpectedly many kubeconfig paths");
    }

    #[test]
    fn loading_from_the_machine_default_either_succeeds_or_reports_why() {
        // The developer machine may or may not have a kubeconfig; both are
        // valid outcomes, and neither may panic.
        match load_kubeconfig() {
            Ok(_) | Err(K8sError::NoKubeconfig | K8sError::MalformedKubeconfig { .. }) => {}
            Err(other) => panic!("unexpected error {other:?}"),
        }
    }
}
