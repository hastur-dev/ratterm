//! SSH credential storage.
//!
//! Host records live in `~/.ratterm/ssh_hosts.toml`. Passwords and key
//! passphrases do not: the file holds a reference such as
//! `secret:ssh/5/password` and the secret itself sits in the backend chosen by
//! [`SecretBackend`] — the OS keychain by default, an Argon2id +
//! XChaCha20-Poly1305 file where no keychain exists.
//!
//! What this replaces: the previous "encrypted" mode derived a key with a
//! hand-written mixing loop and XORed the password against it, with source
//! comments admitting both were placeholders. Files written that way are still
//! readable — [`SSHStorage::set_master_password`] derives the legacy key so the
//! old secrets can be migrated into the new backend — but nothing is ever
//! written in that form again.

use super::host::SSHHostList;
use crate::secrets::{
    KdfParams, SecretBackend, SecretsError, SecretsManager, is_secret_ref, ssh_password_id,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zeroize::Zeroizing;

/// Maximum file size for SSH hosts file (1MB).
const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// Iteration count used by the retired key-derivation loop.
///
/// Kept only so credentials written by earlier versions can be read once and
/// migrated.
const LEGACY_ITERATIONS: u32 = 100_000;

/// Salt length used by the retired scheme, and by nothing else.
const LEGACY_SALT_LENGTH: usize = 32;

/// Prefix marking a value encrypted by the retired XOR scheme.
const LEGACY_PREFIX: &str = "enc:";

/// Storage mode for SSH credentials.
///
/// An alias for [`SecretBackend`]; the old name is kept because it is what the
/// configuration file and the settings UI call it.
pub type StorageMode = SecretBackend;

/// Errors that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// File I/O error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// TOML parsing error.
    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),

    /// TOML serialization error.
    #[error("Serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),

    /// File too large.
    #[error("File too large (max {MAX_FILE_SIZE} bytes)")]
    FileTooLarge,

    /// Invalid master password.
    #[error("Invalid master password")]
    InvalidPassword,

    /// The secret backend failed.
    #[error(transparent)]
    Secrets(#[from] SecretsError),

    /// Master password required but not provided.
    #[error("Master password required")]
    PasswordRequired,
}

/// Storage configuration persisted in the hosts file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StorageConfig {
    /// Storage mode setting.
    storage_mode: StorageMode,
    /// Salt used by the retired key-derivation scheme (base64).
    #[serde(skip_serializing_if = "Option::is_none")]
    encryption_salt: Option<String>,
}

/// Complete storage file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageFile {
    /// Storage settings.
    settings: StorageConfig,
    /// Host list data.
    #[serde(flatten)]
    hosts: SSHHostList,
}

impl Default for StorageFile {
    fn default() -> Self {
        Self {
            settings: StorageConfig::default(),
            hosts: SSHHostList::new(),
        }
    }
}

/// SSH storage manager.
///
/// Handles loading and saving SSH hosts, delegating every secret to
/// [`SecretsManager`].
///
/// `Debug` is written by hand: a derived one would print the session
/// passphrase and the retired key.
pub struct SSHStorage {
    /// Path to the storage file.
    path: PathBuf,
    /// Where secrets are kept.
    secrets: SecretsManager,
    /// Key derived by the retired scheme, for one-way migration only.
    legacy_key: Option<[u8; 32]>,
    /// Salt recorded by the retired scheme.
    ///
    /// Only known after the host file has been parsed, which is why the
    /// passphrase is kept: the master password arrives before the salt does.
    legacy_salt: Option<[u8; LEGACY_SALT_LENGTH]>,
    /// Master passphrase for this session, held so the retired key can be
    /// derived once the salt is read from the file.
    master_passphrase: Option<Zeroizing<String>>,
    /// Whether the storage has been initialized.
    initialized: bool,
    /// Number of secrets moved out of the host file on the last load.
    migrated: usize,
}

impl std::fmt::Debug for SSHStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSHStorage")
            .field("path", &self.path)
            .field("secrets", &self.secrets)
            .field("has_legacy_key", &self.legacy_key.is_some())
            .field("initialized", &self.initialized)
            .field("migrated", &self.migrated)
            .finish()
    }
}

impl SSHStorage {
    /// Creates a new storage manager with the default path and backend.
    #[must_use]
    pub fn new() -> Self {
        Self::with_path(Self::default_path())
    }

    /// Creates a storage manager with a custom path.
    ///
    /// The vault, if one is needed, sits next to the host file so a test or a
    /// portable install keeps everything in one directory.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        assert!(!path.as_os_str().is_empty(), "path must not be empty");

        let vault_path = Self::vault_path_for(&path);
        let secrets = SecretsManager::new(
            SecretBackend::default(),
            vault_path,
            KdfParams::interactive(),
        )
        .unwrap_or_else(|_| Self::fallback_manager());

        Self {
            path,
            secrets,
            legacy_key: None,
            legacy_salt: None,
            master_passphrase: None,
            initialized: false,
            migrated: 0,
        }
    }

    /// Creates a storage manager with an explicit secrets backend.
    ///
    /// Used by tests, which must not touch the developer's real keychain.
    #[must_use]
    pub fn with_path_and_secrets(path: PathBuf, secrets: SecretsManager) -> Self {
        assert!(!path.as_os_str().is_empty(), "path must not be empty");
        Self {
            path,
            secrets,
            legacy_key: None,
            legacy_salt: None,
            master_passphrase: None,
            initialized: false,
            migrated: 0,
        }
    }

    /// A manager that cannot fail to construct, used when the real one does.
    fn fallback_manager() -> SecretsManager {
        // `Vault::open` on a path that does not exist only needs randomness,
        // so this cannot realistically fail; if it does, plaintext is the only
        // remaining behaviour and the caller can see it via `mode()`.
        SecretsManager::new(
            SecretBackend::Plaintext,
            std::env::temp_dir().join("ratterm-unavailable.vault"),
            KdfParams::interactive(),
        )
        .unwrap_or_else(|_| unreachable!("plaintext manager construction cannot fail"))
    }

    /// Returns the vault path that pairs with a host-file path.
    fn vault_path_for(hosts_path: &Path) -> PathBuf {
        hosts_path.parent().map_or_else(
            || PathBuf::from("secrets.vault"),
            |p| p.join("secrets.vault"),
        )
    }

    /// Returns the default storage path.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("ssh_hosts.toml")
    }

    /// Returns the current storage mode.
    #[must_use]
    pub fn mode(&self) -> StorageMode {
        self.secrets.backend()
    }

    /// Sets the storage mode.
    ///
    /// # Errors
    /// Returns an error if the secrets backend cannot be reopened.
    pub fn set_mode(&mut self, mode: StorageMode) -> Result<(), StorageError> {
        let vault_path = self.secrets.vault_path().to_path_buf();
        self.secrets = SecretsManager::new(mode, vault_path, KdfParams::interactive())?;
        Ok(())
    }

    /// Returns true if a master password is required to load.
    #[must_use]
    pub fn needs_master_password(&self) -> bool {
        self.secrets.needs_passphrase()
    }

    /// Returns the number of secrets migrated out of the host file on the last
    /// load.
    #[must_use]
    pub const fn migrated_count(&self) -> usize {
        self.migrated
    }

    /// Supplies the master password.
    ///
    /// This unlocks the encrypted-file backend and, when the host file was
    /// written by an earlier version, also derives the retired key so those
    /// credentials can be read once and migrated.
    ///
    /// # Errors
    /// Returns an error if the passphrase is empty or does not match.
    pub fn set_master_password(&mut self, password: &str) -> Result<(), StorageError> {
        if password.is_empty() {
            return Err(StorageError::InvalidPassword);
        }

        self.master_passphrase = Some(Zeroizing::new(password.to_string()));
        self.derive_legacy_key();

        if self.secrets.backend().needs_passphrase() {
            self.secrets.unlock(password)?;
        }

        Ok(())
    }

    /// Derives the retired key when both the passphrase and the salt are known.
    fn derive_legacy_key(&mut self) {
        if self.legacy_key.is_some() {
            return;
        }
        if let (Some(passphrase), Some(salt)) =
            (self.master_passphrase.as_ref(), self.legacy_salt.as_ref())
        {
            self.legacy_key = Some(legacy_derive_key(passphrase.as_bytes(), salt));
        }
    }

    /// Loads the host list from storage.
    ///
    /// Secrets are resolved so the returned list carries usable credentials.
    /// Any credential still stored in the clear (or under the retired XOR
    /// scheme) is moved into the backend and the host file rewritten, so the
    /// migration happens once and silently.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&mut self) -> Result<SSHHostList, StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !self.path.exists() {
            self.initialized = true;
            return Ok(SSHHostList::new());
        }

        let metadata = fs::metadata(&self.path)?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(StorageError::FileTooLarge);
        }

        let content = fs::read_to_string(&self.path)?;
        let storage_file: StorageFile = toml::from_str(&content)?;

        // The file records which backend wrote it; honour that over the
        // default so a machine that chose plaintext keeps working.
        if storage_file.settings.storage_mode != self.secrets.backend() {
            self.set_mode(storage_file.settings.storage_mode)?;
        }

        self.legacy_salt = storage_file
            .settings
            .encryption_salt
            .as_deref()
            .and_then(decode_legacy_salt);
        // The salt only becomes known here, after the file has been parsed, so
        // a passphrase supplied earlier is applied now.
        self.derive_legacy_key();

        if self.secrets.needs_passphrase() && self.file_has_secrets(&storage_file.hosts) {
            return Err(StorageError::PasswordRequired);
        }

        let mut hosts = storage_file.hosts;
        self.migrated = self.resolve_and_migrate(&mut hosts)?;

        if self.migrated > 0 {
            // Persist the references so the plaintext leaves the file.
            self.save(&hosts)?;
        }

        self.initialized = true;
        Ok(hosts)
    }

    /// Returns true if any credential in `hosts` needs the backend to read it.
    fn file_has_secrets(&self, hosts: &SSHHostList) -> bool {
        hosts.hosts().any(|h| {
            hosts.get_credentials(h.id).is_some_and(|c| {
                c.password
                    .as_deref()
                    .is_some_and(|p| is_secret_ref(p) || p.starts_with(LEGACY_PREFIX))
            })
        })
    }

    /// Replaces stored references with usable secrets, adopting anything still
    /// held in the clear. Returns how many credentials were migrated.
    fn resolve_and_migrate(&mut self, hosts: &mut SSHHostList) -> Result<usize, StorageError> {
        let host_ids: Vec<u32> = hosts.hosts().map(|h| h.id).collect();
        let mut migrated = 0;

        for host_id in host_ids {
            let Some(stored) = hosts
                .get_credentials(host_id)
                .and_then(|c| c.password.clone())
            else {
                continue;
            };

            let resolved = if let Some(legacy) = stored.strip_prefix(LEGACY_PREFIX) {
                match self.decrypt_legacy(legacy) {
                    Some(plain) => Some(plain),
                    // Without the old master password the ciphertext stays put
                    // rather than being destroyed.
                    None => continue,
                }
            } else if is_secret_ref(&stored) {
                self.secrets
                    .resolve(&stored)?
                    .map(|s| String::from(s.as_str()))
            } else {
                Some(stored.clone())
            };

            let Some(plaintext) = resolved else {
                // A dangling reference: leave it alone so the user can see the
                // credential is missing rather than silently losing the host.
                continue;
            };

            let was_in_the_clear = !is_secret_ref(&stored);
            if was_in_the_clear && !self.secrets.is_plaintext() {
                migrated += 1;
            }

            if let Some(creds) = hosts.get_credentials_mut(host_id) {
                creds.password = Some(plaintext);
            }
        }

        Ok(migrated)
    }

    /// Decrypts a value written by the retired XOR scheme.
    fn decrypt_legacy(&self, encoded: &str) -> Option<String> {
        let key = self.legacy_key.as_ref()?;
        let bytes = crate::secrets::vault::b64_decode_public(encoded).ok()?;
        let plain: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();
        String::from_utf8(plain).ok()
    }

    /// Saves the host list to storage.
    ///
    /// Secrets go to the backend; the file gets references.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written or the backend rejects a
    /// secret.
    pub fn save(&mut self, hosts: &SSHHostList) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let hosts_to_save = self.externalise_secrets(hosts)?;

        let settings = StorageConfig {
            storage_mode: self.secrets.backend(),
            // The retired salt is not carried forward once migration is done.
            encryption_salt: None,
        };

        let storage_file = StorageFile {
            settings,
            hosts: hosts_to_save,
        };

        let content = toml::to_string_pretty(&storage_file)?;

        // Write to a temporary file and rename, so an interrupted save cannot
        // truncate the host list.
        let temp_path = self.path.with_extension("tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
        }
        fs::rename(&temp_path, &self.path)?;
        crate::secrets::vault::restrict_permissions_public(&self.path)?;

        Ok(())
    }

    /// Returns a copy of `hosts` with every password replaced by a reference.
    fn externalise_secrets(&mut self, hosts: &SSHHostList) -> Result<SSHHostList, StorageError> {
        let mut out = hosts.clone();

        if self.secrets.is_plaintext() {
            return Ok(out);
        }

        let host_ids: Vec<u32> = out.hosts().map(|h| h.id).collect();
        for host_id in host_ids {
            let Some(password) = out
                .get_credentials(host_id)
                .and_then(|c| c.password.clone())
            else {
                continue;
            };

            if is_secret_ref(&password) {
                continue;
            }

            let id = ssh_password_id(host_id);
            let reference = self.secrets.adopt(&id, &password)?;
            if let Some(creds) = out.get_credentials_mut(host_id) {
                creds.password = Some(reference);
            }
        }

        Ok(out)
    }

    /// Removes the stored secret for a host.
    ///
    /// # Errors
    /// Returns an error if the backend rejects the delete.
    pub fn forget_host_secret(&mut self, host_id: u32) -> Result<bool, StorageError> {
        Ok(self.secrets.remove(&ssh_password_id(host_id))?)
    }

    /// Returns true if storage has been initialized.
    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns true if the storage file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Returns the storage file path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the secrets manager.
    #[must_use]
    pub const fn secrets(&self) -> &SecretsManager {
        &self.secrets
    }
}

/// Decodes the salt recorded by the retired scheme.
fn decode_legacy_salt(encoded: &str) -> Option<[u8; LEGACY_SALT_LENGTH]> {
    let bytes = crate::secrets::vault::b64_decode_public(encoded).ok()?;
    if bytes.len() != LEGACY_SALT_LENGTH {
        return None;
    }
    let mut salt = [0u8; LEGACY_SALT_LENGTH];
    salt.copy_from_slice(&bytes);
    Some(salt)
}

/// Reproduces the retired key-derivation loop.
///
/// This is not a KDF in any meaningful sense; it exists so credentials written
/// by earlier versions can be read once and moved somewhere safe.
fn legacy_derive_key(password: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut state = [0u8; 32];

    for (i, byte) in password.iter().enumerate() {
        state[i % 32] ^= byte;
    }
    for (i, byte) in salt.iter().enumerate() {
        state[(i + 16) % 32] ^= byte;
    }

    for iteration in 0..LEGACY_ITERATIONS {
        for i in 0..32 {
            let idx = (i + iteration as usize) % 32;
            state[i] = state[i]
                .wrapping_add(state[idx])
                .wrapping_mul(17)
                .wrapping_add((iteration & 0xFF) as u8);
        }
    }

    state
}

impl Default for SSHStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::secrets::SecretsManager;
    use crate::ssh::host::SSHCredentials;
    use tempfile::TempDir;

    /// Builds storage backed by a private vault, so tests never touch the
    /// developer's real keychain.
    fn vault_storage(dir: &TempDir) -> SSHStorage {
        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("secrets");
        let mut storage =
            SSHStorage::with_path_and_secrets(dir.path().join("ssh_hosts.toml"), secrets);
        storage
            .set_master_password("test-passphrase")
            .expect("unlock");
        storage
    }

    fn plaintext_storage(dir: &TempDir) -> SSHStorage {
        let secrets = SecretsManager::new(
            SecretBackend::Plaintext,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("secrets");
        SSHStorage::with_path_and_secrets(dir.path().join("ssh_hosts.toml"), secrets)
    }

    fn hosts_with_password(password: &str) -> (SSHHostList, u32) {
        let mut hosts = SSHHostList::new();
        let id = hosts.add_host("192.168.1.100".to_string(), 22).unwrap();
        hosts.set_credentials(
            id,
            SSHCredentials::new("admin".to_string(), Some(password.to_string())),
        );
        (hosts, id)
    }

    #[test]
    fn storage_mode_parse() {
        assert_eq!(StorageMode::parse("plaintext"), StorageMode::Plaintext);
        assert_eq!(StorageMode::parse("masterpass"), StorageMode::EncryptedFile);
        assert_eq!(StorageMode::parse("keychain"), StorageMode::Keychain);
        assert_eq!(StorageMode::parse("external"), StorageMode::Keychain);
    }

    #[test]
    fn the_default_backend_is_not_plaintext() {
        assert_ne!(StorageMode::default(), StorageMode::Plaintext);
    }

    #[test]
    fn hosts_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);

        let mut hosts = SSHHostList::new();
        hosts.add_host("test.example.com".to_string(), 22).unwrap();
        storage.save(&hosts).unwrap();

        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get_by_index(0).unwrap().hostname, "test.example.com");
    }

    #[test]
    fn a_missing_file_loads_as_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);
        assert!(storage.load().unwrap().is_empty());
        assert!(storage.is_initialized());
    }

    #[test]
    fn the_password_never_reaches_the_host_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);
        let (hosts, _id) = hosts_with_password("super-secret");

        storage.save(&hosts).unwrap();

        let raw = std::fs::read_to_string(dir.path().join("ssh_hosts.toml")).unwrap();
        assert!(
            !raw.contains("super-secret"),
            "the host file must not contain the password:\n{raw}"
        );
        assert!(raw.contains("secret:ssh/"), "expected a reference:\n{raw}");
    }

    #[test]
    fn credentials_survive_a_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);
        let (hosts, id) = hosts_with_password("hunter2");

        storage.save(&hosts).unwrap();
        let loaded = storage.load().unwrap();

        let creds = loaded.get_credentials(id).expect("credentials");
        assert_eq!(creds.username, "admin");
        assert_eq!(creds.password.as_deref(), Some("hunter2"));
    }

    #[test]
    fn saving_twice_does_not_double_wrap_the_reference() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);
        let (hosts, id) = hosts_with_password("hunter2");

        storage.save(&hosts).unwrap();
        let loaded = storage.load().unwrap();
        storage.save(&loaded).unwrap();

        let raw = std::fs::read_to_string(dir.path().join("ssh_hosts.toml")).unwrap();
        assert_eq!(raw.matches("secret:").count(), 1, "{raw}");
        assert_eq!(
            storage
                .load()
                .unwrap()
                .get_credentials(id)
                .unwrap()
                .password,
            Some("hunter2".to_string())
        );
    }

    #[test]
    fn a_plaintext_file_is_migrated_into_the_backend_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");

        // A file exactly as an older version wrote it.
        std::fs::write(
            &hosts_path,
            r#"
next_id = 6

[settings]
storage_mode = "plaintext"

[[hosts]]
id = 5
hostname = "10.0.0.18"
port = 22
connection_count = 0

[credentials.5]
username = "hastur"
password = "legacy-plaintext"
save = true
"#,
        )
        .unwrap();

        // Open it with a vault backend, as a user switching modes would.
        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("secrets");
        let mut storage = SSHStorage::with_path_and_secrets(hosts_path.clone(), secrets);
        storage.set_master_password("p").unwrap();

        // The file says plaintext, so that mode is honoured and nothing moves.
        let loaded = storage.load().unwrap();
        assert_eq!(storage.mode(), StorageMode::Plaintext);
        assert_eq!(
            loaded.get_credentials(5).unwrap().password.as_deref(),
            Some("legacy-plaintext")
        );

        // Switching the mode explicitly migrates on the next save+load.
        storage.set_mode(StorageMode::EncryptedFile).unwrap();
        storage.set_master_password("p").unwrap();
        storage.save(&loaded).unwrap();

        let raw = std::fs::read_to_string(&hosts_path).unwrap();
        assert!(!raw.contains("legacy-plaintext"), "{raw}");
        assert_eq!(
            storage.load().unwrap().get_credentials(5).unwrap().password,
            Some("legacy-plaintext".to_string())
        );
    }

    #[test]
    fn the_plaintext_backend_still_writes_passwords_inline() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = plaintext_storage(&dir);
        let (hosts, id) = hosts_with_password("kept-in-the-clear");

        storage.save(&hosts).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ssh_hosts.toml")).unwrap();
        assert!(raw.contains("kept-in-the-clear"));

        let loaded = storage.load().unwrap();
        assert_eq!(
            loaded.get_credentials(id).unwrap().password.as_deref(),
            Some("kept-in-the-clear")
        );
        assert_eq!(storage.migrated_count(), 0);
    }

    #[test]
    fn a_file_written_by_the_retired_xor_scheme_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");

        // Reproduce exactly what the old code wrote.
        let salt = [7u8; LEGACY_SALT_LENGTH];
        let key = legacy_derive_key(b"old-master", &salt);
        let ciphertext: Vec<u8> = b"legacy-secret"
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();
        let salt_b64 = crate::secrets::vault::b64_encode_public(&salt);
        let ct_b64 = crate::secrets::vault::b64_encode_public(&ciphertext);

        std::fs::write(
            &hosts_path,
            format!(
                r#"
next_id = 2

[settings]
storage_mode = "masterpass"
encryption_salt = "{salt_b64}"

[[hosts]]
id = 1
hostname = "10.0.0.18"
port = 22
connection_count = 0

[credentials.1]
username = "hastur"
password = "enc:{ct_b64}"
save = true
"#
            ),
        )
        .unwrap();

        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .expect("secrets");
        let mut storage = SSHStorage::with_path_and_secrets(hosts_path.clone(), secrets);
        storage.set_master_password("old-master").unwrap();

        let loaded = storage.load().unwrap();
        assert_eq!(
            loaded.get_credentials(1).unwrap().password.as_deref(),
            Some("legacy-secret"),
            "the retired ciphertext must be readable once"
        );

        let raw = std::fs::read_to_string(&hosts_path).unwrap();
        assert!(!raw.contains("enc:"), "the XOR value must be gone:\n{raw}");
        assert!(raw.contains("secret:ssh/1/password"), "{raw}");
        assert!(storage.migrated_count() >= 1);
    }

    #[test]
    fn a_retired_ciphertext_is_left_alone_without_the_old_password() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");
        std::fs::write(
            &hosts_path,
            r#"
next_id = 2

[settings]
storage_mode = "plaintext"

[[hosts]]
id = 1
hostname = "10.0.0.18"
port = 22
connection_count = 0

[credentials.1]
username = "hastur"
password = "enc:AAAA"
save = true
"#,
        )
        .unwrap();

        let mut storage = plaintext_storage(&dir);
        let loaded = storage.load().unwrap();
        assert_eq!(
            loaded.get_credentials(1).unwrap().password.as_deref(),
            Some("enc:AAAA"),
            "an unreadable secret must not be destroyed"
        );
    }

    #[test]
    fn an_empty_master_password_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .unwrap();
        let mut storage = SSHStorage::with_path_and_secrets(dir.path().join("h.toml"), secrets);
        assert!(matches!(
            storage.set_master_password(""),
            Err(StorageError::InvalidPassword)
        ));
    }

    #[test]
    fn loading_an_encrypted_file_without_a_passphrase_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");

        {
            let mut storage = vault_storage(&dir);
            let (hosts, _) = hosts_with_password("x");
            storage.save(&hosts).unwrap();
        }

        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .unwrap();
        let mut locked = SSHStorage::with_path_and_secrets(hosts_path, secrets);
        assert!(matches!(locked.load(), Err(StorageError::PasswordRequired)));
    }

    #[test]
    fn a_dangling_reference_is_preserved_rather_than_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");
        std::fs::write(
            &hosts_path,
            r#"
next_id = 2

[settings]
storage_mode = "encrypted"

[[hosts]]
id = 1
hostname = "10.0.0.18"
port = 22
connection_count = 0

[credentials.1]
username = "hastur"
password = "secret:ssh/1/password"
save = true
"#,
        )
        .unwrap();

        let secrets = SecretsManager::new(
            SecretBackend::EncryptedFile,
            dir.path().join("secrets.vault"),
            KdfParams::fast_insecure(),
        )
        .unwrap();
        let mut storage = SSHStorage::with_path_and_secrets(hosts_path, secrets);
        storage.set_master_password("p").unwrap();

        let loaded = storage.load().unwrap();
        assert_eq!(
            loaded.get_credentials(1).unwrap().password.as_deref(),
            Some("secret:ssh/1/password"),
            "a missing secret leaves the reference visible"
        );
    }

    #[test]
    fn forgetting_a_host_secret_removes_it_from_the_backend() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = vault_storage(&dir);
        let (hosts, id) = hosts_with_password("gone-soon");
        storage.save(&hosts).unwrap();

        assert!(storage.forget_host_secret(id).unwrap());
        assert!(!storage.forget_host_secret(id).unwrap());

        let loaded = storage.load().unwrap();
        assert_eq!(
            loaded.get_credentials(id).unwrap().password.as_deref(),
            Some("secret:ssh/1/password")
        );
    }

    #[test]
    fn an_oversized_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_path = dir.path().join("ssh_hosts.toml");
        std::fs::write(&hosts_path, "x".repeat((MAX_FILE_SIZE + 1) as usize)).unwrap();

        let mut storage = plaintext_storage(&dir);
        assert!(matches!(storage.load(), Err(StorageError::FileTooLarge)));
    }

    #[test]
    fn a_damaged_file_is_reported_as_a_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ssh_hosts.toml"), "not [ toml").unwrap();
        let mut storage = plaintext_storage(&dir);
        assert!(matches!(storage.load(), Err(StorageError::Parse(_))));
    }

    #[test]
    fn the_legacy_derivation_is_deterministic() {
        let a = legacy_derive_key(b"pw", &[1u8; LEGACY_SALT_LENGTH]);
        let b = legacy_derive_key(b"pw", &[1u8; LEGACY_SALT_LENGTH]);
        assert_eq!(a, b, "migration depends on reproducing the old key exactly");
    }

    #[test]
    fn the_legacy_derivation_discards_its_inputs() {
        // Documents how bad the retired scheme was, and why nothing is ever
        // written with it again: after 100k rounds of the mixing loop the
        // output no longer depends on the salt at all, so every vault on every
        // machine shared a key space of one per password prefix. Two different
        // salts, and even two different passwords, collapse to the same key.
        let salt_a = legacy_derive_key(b"pw", &[1u8; LEGACY_SALT_LENGTH]);
        let salt_b = legacy_derive_key(b"pw", &[2u8; LEGACY_SALT_LENGTH]);
        assert_eq!(salt_a, salt_b, "the retired loop ignores the salt");

        let other_password = legacy_derive_key(b"completely-different", &[1u8; LEGACY_SALT_LENGTH]);
        assert_eq!(
            salt_a, other_password,
            "the retired loop ignores the password too"
        );
    }

    #[test]
    fn a_malformed_legacy_salt_is_ignored() {
        assert!(decode_legacy_salt("AA==").is_none());
        assert!(decode_legacy_salt("!!not base64!!").is_none());
    }

    #[test]
    fn parse_real_file_format() {
        let toml_content = r#"
next_id = 9

[settings]
storage_mode = "plaintext"

[[hosts]]
id = 5
hostname = "10.0.0.18"
port = 22
display_name = "Desk Rock5c"
last_connected = "1766959123"
connection_count = 2

[[hosts]]
id = 6
hostname = "10.0.0.19"
port = 22
display_name = "Ai Rock5c"
connection_count = 0

[credentials.5]
username = "hastur"
password = "secret123"
save = true

[credentials.6]
username = "hastur"
password = "secret456"
save = true
"#;

        let storage_file: StorageFile = toml::from_str(toml_content).unwrap();
        assert_eq!(storage_file.hosts.len(), 2);
        assert_eq!(
            storage_file.hosts.get_credentials(5).unwrap().password,
            Some("secret123".to_string())
        );
        assert_eq!(
            storage_file.hosts.get_credentials(6).unwrap().password,
            Some("secret456".to_string())
        );
        assert_eq!(
            storage_file.settings.storage_mode,
            StorageMode::Plaintext,
            "an existing plaintext file keeps its declared mode"
        );
    }
}
