//! Secret storage.
//!
//! One entry point for every password and passphrase the application holds,
//! with two backends:
//!
//! - [`keychain::KeychainStore`] — the operating system's credential store.
//!   This is the default: the secret never lands in a file ratterm owns.
//! - [`vault::Vault`] — an Argon2id + XChaCha20-Poly1305 file, for machines
//!   with no usable keychain.
//!
//! A third mode, [`SecretBackend::Plaintext`], keeps working for existing
//! installations that chose it, but it is no longer the default and callers
//! can see from [`SecretsManager::is_plaintext`] that secrets are unprotected.
//!
//! Secrets are addressed by a stable id such as `ssh/5/password`. Host records
//! store that id, not the secret, so editing a host cannot leak one and two
//! records cannot disagree about the same credential.

pub mod keychain;
pub mod vault;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

pub use keychain::{KeychainError, KeychainStore};
pub use vault::{KdfParams, Vault, VaultError};

/// Prefix marking a stored value as a reference rather than a secret.
pub const SECRET_REF_PREFIX: &str = "secret:";

/// Which backend holds the secrets.
///
/// The serialised names are what appears in configuration files. The aliases
/// keep files written by earlier versions readable: `masterpass` was the old
/// name for the encrypted file, and `external` was a placeholder for "a
/// password manager", which is what the OS keychain is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretBackend {
    /// Operating system credential store.
    #[default]
    #[serde(alias = "external", alias = "manager", alias = "os")]
    Keychain,
    /// Passphrase-protected file.
    #[serde(
        rename = "encrypted",
        alias = "masterpass",
        alias = "masterpassword",
        alias = "vault"
    )]
    EncryptedFile,
    /// No protection; secrets sit in the host file in the clear.
    Plaintext,
}

impl SecretBackend {
    /// Parses a backend name, accepting the historical spellings.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "plaintext" | "plain" | "text" => Self::Plaintext,
            "encrypted" | "encryptedfile" | "vault" | "masterpass" | "masterpassword"
            | "master" => Self::EncryptedFile,
            // "external" was the old placeholder for a password manager; the
            // OS keychain is what it was meant to become.
            _ => Self::Keychain,
        }
    }

    /// Returns the canonical name written to configuration.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keychain => "keychain",
            Self::EncryptedFile => "encrypted",
            Self::Plaintext => "plaintext",
        }
    }

    /// Returns a description for the settings UI.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Keychain => {
                "OS keychain (Windows Credential Manager / Keychain / Secret Service)"
            }
            Self::EncryptedFile => {
                "Encrypted file (Argon2id + XChaCha20-Poly1305, passphrase once per session)"
            }
            Self::Plaintext => "Plain text (no protection; not recommended)",
        }
    }

    /// Returns true if this backend needs a passphrase before use.
    #[must_use]
    pub const fn needs_passphrase(self) -> bool {
        matches!(self, Self::EncryptedFile)
    }
}

/// Errors raised by the secrets manager.
#[derive(Debug, Error)]
pub enum SecretsError {
    /// The file vault failed.
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// The OS keychain failed.
    #[error(transparent)]
    Keychain(#[from] KeychainError),

    /// A passphrase is needed but has not been supplied.
    #[error("the secret vault is locked; a passphrase is required")]
    Locked,

    /// The active backend cannot store secrets.
    #[error("the plaintext backend does not store secrets separately")]
    NotApplicable,
}

/// Builds the canonical id for a host's password.
#[must_use]
pub fn ssh_password_id(host_id: u32) -> String {
    format!("ssh/{host_id}/password")
}

/// Builds the canonical id for a host's key passphrase.
#[must_use]
pub fn ssh_key_passphrase_id(host_id: u32) -> String {
    format!("ssh/{host_id}/key-passphrase")
}

/// Returns true if `value` is a reference to a stored secret.
#[must_use]
pub fn is_secret_ref(value: &str) -> bool {
    value.starts_with(SECRET_REF_PREFIX)
}

/// Wraps an id as the reference written into host records.
#[must_use]
pub fn to_secret_ref(id: &str) -> String {
    format!("{SECRET_REF_PREFIX}{id}")
}

/// Extracts the id from a reference, if it is one.
#[must_use]
pub fn secret_ref_id(value: &str) -> Option<&str> {
    value.strip_prefix(SECRET_REF_PREFIX)
}

/// Routes secrets to the configured backend.
pub struct SecretsManager {
    backend: SecretBackend,
    keychain: KeychainStore,
    vault: Vault,
}

impl std::fmt::Debug for SecretsManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretsManager")
            .field("backend", &self.backend)
            .field("vault", &self.vault)
            .finish()
    }
}

impl SecretsManager {
    /// Default path of the encrypted vault.
    #[must_use]
    pub fn default_vault_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("secrets.vault")
    }

    /// Creates a manager for `backend` with an explicit vault path.
    ///
    /// # Errors
    /// Returns an error if an existing vault file cannot be read.
    pub fn new(
        backend: SecretBackend,
        vault_path: impl Into<PathBuf>,
        kdf: KdfParams,
    ) -> Result<Self, SecretsError> {
        Ok(Self {
            backend,
            keychain: KeychainStore::new(),
            vault: Vault::open(vault_path, kdf)?,
        })
    }

    /// Creates a manager using the default vault path and interactive KDF
    /// parameters.
    ///
    /// # Errors
    /// Returns an error if an existing vault file cannot be read.
    pub fn with_backend(backend: SecretBackend) -> Result<Self, SecretsError> {
        Self::new(
            backend,
            Self::default_vault_path(),
            KdfParams::interactive(),
        )
    }

    /// Overrides the keychain store, so tests can use a private service name.
    pub fn set_keychain(&mut self, keychain: KeychainStore) {
        self.keychain = keychain;
    }

    /// Returns the active backend.
    #[must_use]
    pub const fn backend(&self) -> SecretBackend {
        self.backend
    }

    /// Returns true if secrets are unprotected.
    #[must_use]
    pub const fn is_plaintext(&self) -> bool {
        matches!(self.backend, SecretBackend::Plaintext)
    }

    /// Returns the vault path.
    #[must_use]
    pub fn vault_path(&self) -> &Path {
        self.vault.path()
    }

    /// Returns true if the manager needs a passphrase before it can be used.
    #[must_use]
    pub fn needs_passphrase(&self) -> bool {
        self.backend.needs_passphrase() && self.vault.is_locked()
    }

    /// Returns true if the vault has never been given a passphrase.
    #[must_use]
    pub fn vault_is_new(&self) -> bool {
        self.vault.is_new()
    }

    /// Supplies the vault passphrase.
    ///
    /// # Errors
    /// Returns an error if the passphrase is wrong or empty.
    pub fn unlock(&mut self, passphrase: &str) -> Result<(), SecretsError> {
        self.vault.unlock(passphrase)?;
        Ok(())
    }

    /// Forgets the vault key.
    pub fn lock(&mut self) {
        self.vault.lock();
    }

    /// Chooses the best backend this machine can actually support.
    ///
    /// Prefers the OS keychain, and falls back to the encrypted file when no
    /// credential store answers.
    #[must_use]
    pub fn detect_backend() -> SecretBackend {
        if KeychainStore::new().probe() {
            SecretBackend::Keychain
        } else {
            SecretBackend::EncryptedFile
        }
    }

    /// Stores `secret` under `id`.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the write, or
    /// [`SecretsError::NotApplicable`] under the plaintext backend.
    pub fn put(&mut self, id: &str, secret: &str) -> Result<(), SecretsError> {
        match self.backend {
            SecretBackend::Keychain => Ok(self.keychain.put(id, secret)?),
            SecretBackend::EncryptedFile => {
                if self.vault.is_locked() {
                    return Err(SecretsError::Locked);
                }
                self.vault.put(id, secret)?;
                self.vault.save()?;
                Ok(())
            }
            SecretBackend::Plaintext => Err(SecretsError::NotApplicable),
        }
    }

    /// Reads the secret stored under `id`.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the read.
    pub fn get(&self, id: &str) -> Result<Option<Zeroizing<String>>, SecretsError> {
        match self.backend {
            SecretBackend::Keychain => Ok(self.keychain.get(id)?),
            SecretBackend::EncryptedFile => {
                if self.vault.is_locked() {
                    return Err(SecretsError::Locked);
                }
                Ok(self.vault.get(id)?)
            }
            SecretBackend::Plaintext => Ok(None),
        }
    }

    /// Deletes the secret stored under `id`.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the delete.
    pub fn remove(&mut self, id: &str) -> Result<bool, SecretsError> {
        match self.backend {
            SecretBackend::Keychain => Ok(self.keychain.remove(id)?),
            SecretBackend::EncryptedFile => {
                let removed = self.vault.remove(id);
                if removed {
                    self.vault.save()?;
                }
                Ok(removed)
            }
            SecretBackend::Plaintext => Ok(false),
        }
    }

    /// Moves a secret that is currently sitting in the clear into the backend,
    /// returning the reference to store in its place.
    ///
    /// Under the plaintext backend the value is handed back unchanged, so
    /// migration is a no-op for users who chose it deliberately.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the write.
    pub fn adopt(&mut self, id: &str, plaintext: &str) -> Result<String, SecretsError> {
        if self.is_plaintext() {
            return Ok(plaintext.to_string());
        }
        self.put(id, plaintext)?;
        Ok(to_secret_ref(id))
    }

    /// Resolves a stored value: a reference is looked up, anything else is
    /// returned as-is.
    ///
    /// This is what lets a plaintext host file keep working while new writes
    /// go to the backend.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the read.
    pub fn resolve(&self, stored: &str) -> Result<Option<Zeroizing<String>>, SecretsError> {
        match secret_ref_id(stored) {
            Some(id) => self.get(id),
            None => Ok(Some(Zeroizing::new(stored.to_string()))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn vault_manager(dir: &Path) -> SecretsManager {
        SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("manager")
    }

    #[test]
    fn backend_names_round_trip() {
        for backend in [
            SecretBackend::Keychain,
            SecretBackend::EncryptedFile,
            SecretBackend::Plaintext,
        ] {
            assert_eq!(SecretBackend::parse(backend.as_str()), backend);
        }
    }

    #[test]
    fn historical_backend_names_still_parse() {
        assert_eq!(
            SecretBackend::parse("masterpass"),
            SecretBackend::EncryptedFile
        );
        assert_eq!(
            SecretBackend::parse("masterpassword"),
            SecretBackend::EncryptedFile
        );
        assert_eq!(SecretBackend::parse("plain"), SecretBackend::Plaintext);
        assert_eq!(SecretBackend::parse("external"), SecretBackend::Keychain);
    }

    #[test]
    fn an_unknown_backend_name_falls_back_to_the_keychain() {
        assert_eq!(SecretBackend::parse("nonsense"), SecretBackend::Keychain);
        assert_eq!(SecretBackend::parse(""), SecretBackend::Keychain);
    }

    #[test]
    fn only_the_encrypted_file_needs_a_passphrase() {
        assert!(SecretBackend::EncryptedFile.needs_passphrase());
        assert!(!SecretBackend::Keychain.needs_passphrase());
        assert!(!SecretBackend::Plaintext.needs_passphrase());
    }

    #[test]
    fn every_backend_has_a_description() {
        for backend in [
            SecretBackend::Keychain,
            SecretBackend::EncryptedFile,
            SecretBackend::Plaintext,
        ] {
            assert!(!backend.description().is_empty());
        }
    }

    #[test]
    fn secret_ids_are_stable_and_distinct() {
        assert_eq!(ssh_password_id(5), "ssh/5/password");
        assert_eq!(ssh_key_passphrase_id(5), "ssh/5/key-passphrase");
        assert_ne!(ssh_password_id(5), ssh_key_passphrase_id(5));
        assert_ne!(ssh_password_id(5), ssh_password_id(6));
    }

    #[test]
    fn references_are_recognised_and_unwrapped() {
        let reference = to_secret_ref("ssh/1/password");
        assert!(is_secret_ref(&reference));
        assert_eq!(secret_ref_id(&reference), Some("ssh/1/password"));
        assert!(!is_secret_ref("hunter2"));
        assert_eq!(secret_ref_id("hunter2"), None);
    }

    #[test]
    fn the_vault_backend_stores_and_reads_secrets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        assert!(mgr.needs_passphrase());
        mgr.unlock("passphrase").expect("unlock");
        assert!(!mgr.needs_passphrase());

        mgr.put("ssh/1/password", "hunter2").expect("put");
        assert_eq!(
            mgr.get("ssh/1/password")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("hunter2")
        );
        assert!(mgr.remove("ssh/1/password").expect("remove"));
        assert!(mgr.get("ssh/1/password").expect("get").is_none());
    }

    #[test]
    fn a_locked_vault_backend_refuses_reads_and_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        assert!(matches!(mgr.put("k", "v"), Err(SecretsError::Locked)));
        assert!(matches!(mgr.get("k"), Err(SecretsError::Locked)));
    }

    #[test]
    fn adopt_moves_a_plaintext_secret_and_returns_a_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        mgr.unlock("p").expect("unlock");

        let reference = mgr.adopt("ssh/2/password", "old-plaintext").expect("adopt");
        assert_eq!(reference, "secret:ssh/2/password");
        assert_eq!(
            mgr.get("ssh/2/password")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("old-plaintext")
        );
    }

    #[test]
    fn adopt_is_a_no_op_under_the_plaintext_backend() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = SecretsManager::new(
            SecretBackend::Plaintext,
            dir.path().join("unused.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("manager");

        assert_eq!(mgr.adopt("ssh/1/password", "kept").expect("adopt"), "kept");
        assert!(mgr.is_plaintext());
    }

    #[test]
    fn the_plaintext_backend_refuses_to_store_secrets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = SecretsManager::new(
            SecretBackend::Plaintext,
            dir.path().join("unused.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("manager");
        assert!(matches!(
            mgr.put("k", "v"),
            Err(SecretsError::NotApplicable)
        ));
        assert!(mgr.get("k").expect("get").is_none());
        assert!(!mgr.remove("k").expect("remove"));
    }

    #[test]
    fn resolve_passes_through_a_literal_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        mgr.unlock("p").expect("unlock");
        assert_eq!(
            mgr.resolve("literal-password")
                .expect("resolve")
                .as_deref()
                .map(String::as_str),
            Some("literal-password")
        );
    }

    #[test]
    fn resolve_looks_up_a_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        mgr.unlock("p").expect("unlock");
        let reference = mgr.adopt("ssh/3/password", "stored").expect("adopt");
        assert_eq!(
            mgr.resolve(&reference)
                .expect("resolve")
                .as_deref()
                .map(String::as_str),
            Some("stored")
        );
    }

    #[test]
    fn resolve_returns_none_for_a_dangling_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = vault_manager(dir.path());
        mgr.unlock("p").expect("unlock");
        assert!(
            mgr.resolve("secret:ssh/99/password")
                .expect("resolve")
                .is_none()
        );
    }

    #[test]
    fn vault_secrets_survive_a_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let mut mgr = vault_manager(dir.path());
            mgr.unlock("p").expect("unlock");
            mgr.put("ssh/4/password", "persisted").expect("put");
        }
        let mut mgr = vault_manager(dir.path());
        mgr.unlock("p").expect("unlock");
        assert_eq!(
            mgr.get("ssh/4/password")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("persisted")
        );
    }

    #[test]
    fn detect_backend_never_reports_plaintext() {
        // Whatever this machine has, the automatic choice must be a protected
        // one; plaintext is only ever a deliberate setting.
        assert_ne!(SecretsManager::detect_backend(), SecretBackend::Plaintext);
    }
}
