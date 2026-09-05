//! OS keychain backend.
//!
//! Wraps the platform credential store — Windows Credential Manager, macOS
//! Keychain, and the Secret Service on Linux — behind the same small interface
//! as the file vault, so the rest of the application does not care which one
//! is in use.
//!
//! Availability is a runtime question, not a compile-time one: a Linux box
//! with no session D-Bus has the code compiled in but no store to talk to.
//! [`KeychainStore::probe`] answers that by round-tripping a throwaway entry.

use keyring::Entry;
use thiserror::Error;
use zeroize::Zeroizing;

/// Service name registered with the platform credential store.
pub const SERVICE: &str = "ratterm";

/// Entry used to check whether the platform store actually works.
const PROBE_ACCOUNT: &str = "ratterm-availability-probe";

/// Errors raised by the keychain backend.
#[derive(Debug, Error)]
pub enum KeychainError {
    /// The platform store rejected the operation.
    #[error("OS keychain error: {0}")]
    Backend(String),

    /// No usable platform store on this machine.
    #[error("no OS keychain available on this system")]
    Unavailable,
}

impl KeychainError {
    fn from_keyring(err: &keyring::Error) -> Self {
        match err {
            keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
                Self::Unavailable
            }
            other => Self::Backend(other.to_string()),
        }
    }
}

/// Secrets held in the operating system's credential store.
#[derive(Debug, Clone, Default)]
pub struct KeychainStore {
    /// Service name; overridable so tests do not collide with the real store.
    service: String,
}

impl KeychainStore {
    /// Creates a store using the default service name.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: SERVICE.to_string(),
        }
    }

    /// Creates a store under a custom service name.
    #[must_use]
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// Returns the service name in use.
    #[must_use]
    pub fn service(&self) -> &str {
        &self.service
    }

    fn entry(&self, id: &str) -> Result<Entry, KeychainError> {
        Entry::new(&self.service, id).map_err(|e| KeychainError::from_keyring(&e))
    }

    /// Stores `secret` under `id`.
    ///
    /// # Errors
    /// Returns an error if the platform store rejects the write.
    pub fn put(&self, id: &str, secret: &str) -> Result<(), KeychainError> {
        self.entry(id)?
            .set_password(secret)
            .map_err(|e| KeychainError::from_keyring(&e))
    }

    /// Reads the secret stored under `id`.
    ///
    /// A missing entry is `Ok(None)`, not an error.
    ///
    /// # Errors
    /// Returns an error if the platform store rejects the read.
    pub fn get(&self, id: &str) -> Result<Option<Zeroizing<String>>, KeychainError> {
        match self.entry(id)?.get_password() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeychainError::from_keyring(&e)),
        }
    }

    /// Deletes the secret stored under `id`.
    ///
    /// Returns true if an entry was removed. Deleting an absent entry is not
    /// an error.
    ///
    /// # Errors
    /// Returns an error if the platform store rejects the delete.
    pub fn remove(&self, id: &str) -> Result<bool, KeychainError> {
        match self.entry(id)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(KeychainError::from_keyring(&e)),
        }
    }

    /// Returns true if this machine has a working credential store.
    ///
    /// Writes, reads back, and deletes a throwaway entry. Anything less can be
    /// wrong: on Linux the crate compiles fine but the Secret Service may not
    /// be running.
    #[must_use]
    pub fn probe(&self) -> bool {
        let probe_value = "probe";
        let Ok(entry) = self.entry(PROBE_ACCOUNT) else {
            return false;
        };
        if entry.set_password(probe_value).is_err() {
            return false;
        }
        let round_tripped = matches!(entry.get_password(), Ok(v) if v == probe_value);
        let _ = entry.delete_credential();
        round_tripped
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Unique service name per test so runs never collide, and so a failed
    /// test cannot leave an entry that another test reads.
    fn unique_service(tag: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("ratterm-test-{tag}-{nanos}")
    }

    #[test]
    fn the_default_service_name_is_the_application_name() {
        assert_eq!(KeychainStore::new().service(), "ratterm");
    }

    #[test]
    fn the_service_name_can_be_overridden() {
        let store = KeychainStore::with_service("other");
        assert_eq!(store.service(), "other");
    }

    #[test]
    fn secrets_round_trip_when_a_store_is_available() {
        let store = KeychainStore::with_service(unique_service("roundtrip"));
        if !store.probe() {
            // No credential store on this machine; the file vault covers it.
            return;
        }

        store.put("account", "s3cret").expect("put");
        assert_eq!(
            store
                .get("account")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("s3cret")
        );
        assert!(store.remove("account").expect("remove"));
        assert!(store.get("account").expect("get").is_none());
    }

    #[test]
    fn a_missing_entry_reads_as_none() {
        let store = KeychainStore::with_service(unique_service("missing"));
        if !store.probe() {
            return;
        }
        assert!(store.get("never-written").expect("get").is_none());
    }

    #[test]
    fn removing_a_missing_entry_is_not_an_error() {
        let store = KeychainStore::with_service(unique_service("remove-missing"));
        if !store.probe() {
            return;
        }
        assert!(!store.remove("never-written").expect("remove"));
    }

    #[test]
    fn overwriting_replaces_the_previous_secret() {
        let store = KeychainStore::with_service(unique_service("overwrite"));
        if !store.probe() {
            return;
        }
        store.put("account", "first").expect("put first");
        store.put("account", "second").expect("put second");
        assert_eq!(
            store
                .get("account")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("second")
        );
        let _ = store.remove("account");
    }
}
