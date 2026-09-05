//! Kubernetes preferences kept in `~/.ratterm/k8s.toml`.
//!
//! Only choices belong here — pinned contexts, favourite namespaces, the last
//! selection. Credentials stay in the kubeconfig and in the SSH host store;
//! nothing secret is written to this file.
//!
//! Writes go through [`super::atomic::write_atomic`], so an interrupted save
//! cannot leave a truncated settings file behind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::atomic::write_atomic;
use super::{K8sError, Result};

/// Largest settings file that will be read.
///
/// The file holds a handful of names; anything approaching this size is
/// corrupt or was written by something else.
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Upper bound on how many names are kept in any one list, so a runaway caller
/// cannot grow the file without limit.
const MAX_ENTRIES: usize = 512;

/// The stored preferences.
///
/// Field order matters: `toml` emits tables after values, so the map has to be
/// declared last for the file to round-trip.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct K8sSettings {
    /// The context selected when the Kubernetes screen was last closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_context: Option<String>,

    /// The namespace selected when the Kubernetes screen was last closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_namespace: Option<String>,

    /// Contexts pinned to the top of the context list, sorted.
    #[serde(default)]
    pub pinned_contexts: Vec<String>,

    /// Favourite namespaces per context, each list sorted.
    #[serde(default)]
    pub favourite_namespaces: BTreeMap<String, Vec<String>>,
}

impl K8sSettings {
    /// Pins a context. Returns true when this changed anything.
    pub fn pin_context(&mut self, context: &str) -> bool {
        if context.is_empty() || self.pinned_contexts.len() >= MAX_ENTRIES {
            return false;
        }
        insert_sorted(&mut self.pinned_contexts, context)
    }

    /// Unpins a context. Returns true when this changed anything.
    pub fn unpin_context(&mut self, context: &str) -> bool {
        let before = self.pinned_contexts.len();
        self.pinned_contexts.retain(|c| c != context);
        self.pinned_contexts.len() != before
    }

    /// True when the context is pinned.
    #[must_use]
    pub fn is_pinned(&self, context: &str) -> bool {
        self.pinned_contexts.iter().any(|c| c == context)
    }

    /// Marks a namespace as a favourite of a context. Returns true when this
    /// changed anything.
    pub fn add_favourite_namespace(&mut self, context: &str, namespace: &str) -> bool {
        if context.is_empty() || namespace.is_empty() {
            return false;
        }
        if !self.favourite_namespaces.contains_key(context)
            && self.favourite_namespaces.len() >= MAX_ENTRIES
        {
            return false;
        }
        let list = self
            .favourite_namespaces
            .entry(context.to_string())
            .or_default();
        if list.len() >= MAX_ENTRIES {
            return false;
        }
        insert_sorted(list, namespace)
    }

    /// Removes a favourite namespace. Returns true when this changed anything.
    ///
    /// A context left with no favourites loses its entry, so the file does not
    /// accumulate empty tables.
    pub fn remove_favourite_namespace(&mut self, context: &str, namespace: &str) -> bool {
        let Some(list) = self.favourite_namespaces.get_mut(context) else {
            return false;
        };
        let before = list.len();
        list.retain(|n| n != namespace);
        let changed = list.len() != before;
        if list.is_empty() {
            self.favourite_namespaces.remove(context);
        }
        changed
    }

    /// Returns a context's favourite namespaces, empty when it has none.
    #[must_use]
    pub fn favourite_namespaces(&self, context: &str) -> &[String] {
        self.favourite_namespaces
            .get(context)
            .map_or(&[], Vec::as_slice)
    }

    /// True when the namespace is a favourite of the context.
    #[must_use]
    pub fn is_favourite_namespace(&self, context: &str, namespace: &str) -> bool {
        self.favourite_namespaces(context)
            .iter()
            .any(|n| n == namespace)
    }

    /// Records the current selection.
    pub fn set_last_selection(&mut self, context: &str, namespace: &str) {
        self.last_context = (!context.is_empty()).then(|| context.to_string());
        self.last_namespace = (!namespace.is_empty()).then(|| namespace.to_string());
    }

    /// Drops every entry for contexts that are no longer in the kubeconfig.
    ///
    /// Called after a context list refresh so preferences do not accumulate for
    /// clusters that have been removed.
    pub fn retain_contexts(&mut self, known: &[String]) {
        self.pinned_contexts.retain(|c| known.contains(c));
        self.favourite_namespaces.retain(|c, _| known.contains(c));
        if let Some(last) = self.last_context.as_ref()
            && !known.contains(last)
        {
            self.last_context = None;
            self.last_namespace = None;
        }
    }
}

/// Inserts `value` into a sorted list, keeping it sorted and unique.
///
/// Returns true when the value was not already there.
fn insert_sorted(list: &mut Vec<String>, value: &str) -> bool {
    match list.binary_search_by(|item| item.as_str().cmp(value)) {
        Ok(_) => false,
        Err(index) => {
            list.insert(index, value.to_string());
            true
        }
    }
}

/// Reads and writes [`K8sSettings`].
#[derive(Debug, Clone)]
pub struct K8sStorage {
    path: PathBuf,
}

impl Default for K8sStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl K8sStorage {
    /// Uses the standard location, `~/.ratterm/k8s.toml`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            path: Self::default_path(),
        }
    }

    /// Uses an explicit file. Used by tests and by callers with a custom home.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the standard settings file path.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("k8s.toml")
    }

    /// Returns the file this storage reads and writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// True when the settings file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// Reads the settings.
    ///
    /// A missing file is not an error: it loads as the defaults, which is what
    /// a first run looks like.
    ///
    /// # Errors
    /// [`K8sError::Storage`] if the file exists but cannot be read, is larger
    /// than the size cap, or is not valid TOML.
    pub fn load(&self) -> Result<K8sSettings> {
        if !self.path.exists() {
            return Ok(K8sSettings::default());
        }

        let size = std::fs::metadata(&self.path)
            .map_err(|e| {
                K8sError::Storage(format!("{} could not be read: {e}", self.path.display()))
            })?
            .len();
        if size > MAX_FILE_BYTES {
            return Err(K8sError::Storage(format!(
                "{} is {size} bytes, larger than the {MAX_FILE_BYTES} byte limit; delete it to \
                 start again",
                self.path.display()
            )));
        }

        let text = std::fs::read_to_string(&self.path).map_err(|e| {
            K8sError::Storage(format!("{} could not be read: {e}", self.path.display()))
        })?;

        toml::from_str(&text).map_err(|e| {
            K8sError::Storage(format!(
                "{} is not valid TOML: {e}; delete it to start again",
                self.path.display()
            ))
        })
    }

    /// Writes the settings, replacing whatever was there.
    ///
    /// # Errors
    /// [`K8sError::Storage`] if the file cannot be written.
    pub fn save(&self, settings: &K8sSettings) -> Result<()> {
        let text = toml::to_string_pretty(settings)
            .map_err(|e| K8sError::Storage(format!("the settings could not be encoded: {e}")))?;
        write_atomic(&self.path, text.as_bytes())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn storage(dir: &tempfile::TempDir) -> K8sStorage {
        K8sStorage::with_path(dir.path().join("k8s.toml"))
    }

    fn populated() -> K8sSettings {
        let mut settings = K8sSettings::default();
        settings.pin_context("prod");
        settings.pin_context("dev");
        settings.add_favourite_namespace("prod", "web");
        settings.add_favourite_namespace("prod", "data");
        settings.add_favourite_namespace("dev", "sandbox");
        settings.set_last_selection("prod", "web");
        settings
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);
        let settings = populated();

        store.save(&settings).expect("save");
        assert!(store.exists());
        let loaded = store.load().expect("load");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn a_missing_file_loads_as_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);

        assert!(!store.exists());
        let loaded = store.load().expect("load");
        assert_eq!(loaded, K8sSettings::default());
        assert!(loaded.pinned_contexts.is_empty());
        assert!(loaded.favourite_namespaces.is_empty());
        assert!(loaded.last_context.is_none());
    }

    #[test]
    fn a_malformed_file_names_itself_and_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);
        std::fs::write(store.path(), "pinned_contexts = [ this is not toml").expect("write");

        match store.load() {
            Err(K8sError::Storage(message)) => {
                assert!(message.contains("k8s.toml"), "{message}");
                assert!(message.contains("delete it"), "{message}");
            }
            other => panic!("expected a storage error, got {other:?}"),
        }
    }

    #[test]
    fn an_oversized_file_is_refused_before_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);
        let filler = vec![b'#'; usize::try_from(MAX_FILE_BYTES).unwrap_or(0) + 16];
        std::fs::write(store.path(), &filler).expect("write");

        match store.load() {
            Err(K8sError::Storage(message)) => assert!(message.contains("limit"), "{message}"),
            other => panic!("expected a storage error, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_file_fills_in_the_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);
        std::fs::write(store.path(), "last_context = \"prod\"\n").expect("write");

        let loaded = store.load().expect("load");
        assert_eq!(loaded.last_context.as_deref(), Some("prod"));
        assert!(loaded.last_namespace.is_none());
        assert!(loaded.pinned_contexts.is_empty());
        assert!(loaded.favourite_namespaces.is_empty());
    }

    #[test]
    fn an_atomic_save_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);
        store.save(&populated()).expect("save");

        let entries: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["k8s.toml".to_string()]);
    }

    #[test]
    fn saving_twice_replaces_the_file_rather_than_appending() {
        let dir = tempfile::tempdir().unwrap();
        let store = storage(&dir);

        store.save(&populated()).expect("first save");
        let mut second = K8sSettings::default();
        second.pin_context("only");
        store.save(&second).expect("second save");

        let loaded = store.load().expect("load");
        assert_eq!(loaded.pinned_contexts, vec!["only".to_string()]);
        assert!(loaded.last_context.is_none());
    }

    #[test]
    fn writing_creates_the_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = K8sStorage::with_path(dir.path().join("nested").join("deep").join("k8s.toml"));
        store.save(&K8sSettings::default()).expect("save");
        assert!(store.exists());
    }

    #[test]
    fn pinning_is_sorted_idempotent_and_reversible() {
        let mut settings = K8sSettings::default();
        assert!(settings.pin_context("prod"));
        assert!(settings.pin_context("dev"));
        assert!(
            !settings.pin_context("prod"),
            "pinning twice changes nothing"
        );
        assert_eq!(settings.pinned_contexts, vec!["dev", "prod"]);
        assert!(settings.is_pinned("prod"));

        assert!(settings.unpin_context("prod"));
        assert!(!settings.unpin_context("prod"));
        assert!(!settings.is_pinned("prod"));
        assert_eq!(settings.pinned_contexts, vec!["dev"]);
    }

    #[test]
    fn an_empty_context_name_is_not_pinned() {
        let mut settings = K8sSettings::default();
        assert!(!settings.pin_context(""));
        assert!(settings.pinned_contexts.is_empty());
    }

    #[test]
    fn favourite_namespaces_are_per_context_and_sorted() {
        let mut settings = K8sSettings::default();
        assert!(settings.add_favourite_namespace("prod", "web"));
        assert!(settings.add_favourite_namespace("prod", "data"));
        assert!(!settings.add_favourite_namespace("prod", "web"));
        assert!(settings.add_favourite_namespace("dev", "sandbox"));

        assert_eq!(settings.favourite_namespaces("prod"), ["data", "web"]);
        assert_eq!(settings.favourite_namespaces("dev"), ["sandbox"]);
        assert!(settings.favourite_namespaces("staging").is_empty());
        assert!(settings.is_favourite_namespace("prod", "web"));
        assert!(!settings.is_favourite_namespace("dev", "web"));
    }

    #[test]
    fn removing_the_last_favourite_drops_the_context_entry() {
        let mut settings = K8sSettings::default();
        settings.add_favourite_namespace("prod", "web");

        assert!(settings.remove_favourite_namespace("prod", "web"));
        assert!(settings.favourite_namespaces.is_empty());
        assert!(!settings.remove_favourite_namespace("prod", "web"));
        assert!(!settings.remove_favourite_namespace("absent", "web"));
    }

    #[test]
    fn an_empty_namespace_or_context_is_not_favourited() {
        let mut settings = K8sSettings::default();
        assert!(!settings.add_favourite_namespace("", "web"));
        assert!(!settings.add_favourite_namespace("prod", ""));
        assert!(settings.favourite_namespaces.is_empty());
    }

    #[test]
    fn the_last_selection_is_recorded_and_cleared() {
        let mut settings = K8sSettings::default();
        settings.set_last_selection("prod", "web");
        assert_eq!(settings.last_context.as_deref(), Some("prod"));
        assert_eq!(settings.last_namespace.as_deref(), Some("web"));

        settings.set_last_selection("", "");
        assert!(settings.last_context.is_none());
        assert!(settings.last_namespace.is_none());
    }

    #[test]
    fn preferences_for_removed_contexts_are_dropped() {
        let mut settings = populated();
        settings.retain_contexts(&["dev".to_string()]);

        assert_eq!(settings.pinned_contexts, vec!["dev"]);
        assert!(settings.favourite_namespaces("prod").is_empty());
        assert_eq!(settings.favourite_namespaces("dev"), ["sandbox"]);
        assert!(settings.last_context.is_none());
        assert!(settings.last_namespace.is_none());
    }

    #[test]
    fn a_still_present_last_context_survives_a_refresh() {
        let mut settings = populated();
        settings.retain_contexts(&["prod".to_string(), "dev".to_string()]);
        assert_eq!(settings.last_context.as_deref(), Some("prod"));
    }

    #[test]
    fn the_pinned_list_stops_growing_at_the_cap() {
        let mut settings = K8sSettings::default();
        for index in 0..MAX_ENTRIES {
            assert!(settings.pin_context(&format!("context-{index:04}")));
        }
        assert!(!settings.pin_context("one-too-many"));
        assert_eq!(settings.pinned_contexts.len(), MAX_ENTRIES);
    }

    #[test]
    fn the_default_path_is_under_the_ratterm_directory() {
        let path = K8sStorage::default_path();
        assert!(path.ends_with("k8s.toml"), "{}", path.display());
        assert!(
            path.to_string_lossy().contains(".ratterm"),
            "{}",
            path.display()
        );
        assert_eq!(K8sStorage::new().path(), K8sStorage::default_path());
        assert_eq!(K8sStorage::default().path(), K8sStorage::default_path());
    }
}
