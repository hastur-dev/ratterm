//! Passphrase-protected secret vault.
//!
//! This is the fallback for machines with no usable OS keychain (a headless
//! Linux box with no Secret Service, for example). It replaces the previous
//! "encrypted" mode, which derived a key with a hand-written mixing loop and
//! then XORed the password against it — a scheme that leaks the plaintext to
//! anyone who can guess or observe a single byte pair.
//!
//! Construction:
//!
//! - Key derivation: Argon2id, parameters recorded in the file so they can be
//!   raised later without breaking existing vaults.
//! - Encryption: XChaCha20-Poly1305, a fresh 24-byte nonce per entry per save.
//! - Associated data: the entry's id, so a ciphertext cannot be moved from one
//!   entry to another.
//! - Passphrase check: a verifier entry encrypting a known constant, so a
//!   wrong passphrase is reported as such instead of surfacing as corrupt
//!   secrets.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

/// On-disk format version.
pub const VAULT_VERSION: u32 = 1;

/// Derived key length in bytes.
const KEY_LEN: usize = 32;
/// Salt length in bytes.
const SALT_LEN: usize = 32;
/// XChaCha20-Poly1305 nonce length in bytes.
const NONCE_LEN: usize = 24;
/// Largest vault file we will read (1 MiB is far beyond any real vault).
const MAX_VAULT_BYTES: u64 = 1024 * 1024;

/// Entry id used for the passphrase verifier.
const VERIFIER_ID: &str = "\u{0}verifier";
/// Plaintext encrypted under the verifier entry.
const VERIFIER_PLAINTEXT: &[u8] = b"ratterm-vault-v1";

/// Errors raised by the vault.
#[derive(Debug, Error)]
pub enum VaultError {
    /// File I/O failure.
    #[error("vault I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The vault file could not be parsed.
    #[error("vault file is not valid TOML: {0}")]
    Parse(#[from] toml::de::Error),

    /// The vault could not be serialised.
    #[error("vault could not be serialised: {0}")]
    Serialize(#[from] toml::ser::Error),

    /// The vault file is larger than [`MAX_VAULT_BYTES`].
    #[error("vault file is larger than {MAX_VAULT_BYTES} bytes")]
    FileTooLarge,

    /// Key derivation failed.
    #[error("key derivation failed: {0}")]
    Kdf(String),

    /// Encryption or decryption failed.
    #[error("secret could not be decrypted; the vault may be damaged")]
    Crypto,

    /// An operation needing the key was attempted while locked.
    #[error("vault is locked")]
    Locked,

    /// The supplied passphrase does not match the vault.
    #[error("incorrect passphrase")]
    WrongPassphrase,

    /// An empty passphrase was supplied.
    #[error("passphrase must not be empty")]
    EmptyPassphrase,

    /// The file is structurally invalid.
    #[error("vault file is damaged: {0}")]
    Corrupt(String),

    /// Randomness was unavailable.
    #[error("system randomness unavailable: {0}")]
    Random(String),
}

/// Argon2id parameters, recorded alongside the ciphertext.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Number of passes.
    pub iterations: u32,
    /// Degree of parallelism.
    pub parallelism: u32,
}

impl KdfParams {
    /// Parameters for interactive use.
    ///
    /// 19 MiB / 2 passes / 1 lane is the OWASP minimum for Argon2id and costs
    /// roughly a tenth of a second, which is acceptable for a prompt entered
    /// once per session.
    #[must_use]
    pub const fn interactive() -> Self {
        Self {
            memory_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        }
    }

    /// Deliberately cheap parameters for tests.
    ///
    /// Not exposed to users: a vault written with these is easy to attack
    /// offline. Tests need to unlock hundreds of vaults per run.
    #[must_use]
    pub const fn fast_insecure() -> Self {
        Self {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn to_argon2(self) -> Result<Argon2<'static>, VaultError> {
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|e| VaultError::Kdf(e.to_string()))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// A single encrypted entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedEntry {
    /// Base64 nonce.
    nonce: String,
    /// Base64 ciphertext including the authentication tag.
    ciphertext: String,
}

/// Key-derivation section of the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KdfSection {
    /// Always `argon2id` for version 1.
    algorithm: String,
    /// Base64 salt.
    salt: String,
    /// Cost parameters.
    #[serde(flatten)]
    params: KdfParams,
}

/// The complete vault file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultFile {
    /// Format version.
    version: u32,
    /// Key-derivation settings.
    kdf: KdfSection,
    /// Encrypted entries keyed by id.
    #[serde(default)]
    entries: BTreeMap<String, EncryptedEntry>,
}

fn b64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn b64_decode(text: &str, what: &str) -> Result<Vec<u8>, VaultError> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|e| VaultError::Corrupt(format!("{what} is not valid base64: {e}")))
}

/// Fills `buf` with operating-system randomness.
fn random_bytes(buf: &mut [u8]) -> Result<(), VaultError> {
    getrandom::fill(buf).map_err(|e| VaultError::Random(e.to_string()))
}

/// Standard base64 encoding, shared with the SSH storage migration path.
#[must_use]
pub fn b64_encode_public(bytes: &[u8]) -> String {
    b64_encode(bytes)
}

/// Standard base64 decoding, shared with the SSH storage migration path.
///
/// # Errors
/// Returns an error if `text` is not valid base64.
pub fn b64_decode_public(text: &str) -> Result<Vec<u8>, VaultError> {
    b64_decode(text, "value")
}

/// Restricts a file to its owner; see [`restrict_permissions`].
///
/// # Errors
/// Returns an error if the permissions cannot be changed.
pub fn restrict_permissions_public(path: &Path) -> std::io::Result<()> {
    restrict_permissions(path)
}

/// A passphrase-protected store of named secrets.
pub struct Vault {
    path: PathBuf,
    file: VaultFile,
    key: Option<Zeroizing<[u8; KEY_LEN]>>,
    dirty: bool,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("path", &self.path)
            .field("entries", &self.file.entries.len())
            .field("locked", &self.key.is_none())
            .finish()
    }
}

impl Vault {
    /// Opens the vault at `path`, creating an in-memory empty one if the file
    /// does not exist yet.
    ///
    /// Nothing is written until [`Vault::save`] is called.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn open(path: impl Into<PathBuf>, params: KdfParams) -> Result<Self, VaultError> {
        let path = path.into();

        if !path.exists() {
            let mut salt = [0u8; SALT_LEN];
            random_bytes(&mut salt)?;
            return Ok(Self {
                path,
                file: VaultFile {
                    version: VAULT_VERSION,
                    kdf: KdfSection {
                        algorithm: "argon2id".to_string(),
                        salt: b64_encode(&salt),
                        params,
                    },
                    entries: BTreeMap::new(),
                },
                key: None,
                dirty: false,
            });
        }

        let size = fs::metadata(&path)?.len();
        if size > MAX_VAULT_BYTES {
            return Err(VaultError::FileTooLarge);
        }

        let text = fs::read_to_string(&path)?;
        let file: VaultFile = toml::from_str(&text)?;

        if file.version != VAULT_VERSION {
            return Err(VaultError::Corrupt(format!(
                "unsupported vault version {}",
                file.version
            )));
        }
        if file.kdf.algorithm != "argon2id" {
            return Err(VaultError::Corrupt(format!(
                "unsupported key derivation {:?}",
                file.kdf.algorithm
            )));
        }

        Ok(Self {
            path,
            file,
            key: None,
            dirty: false,
        })
    }

    /// Returns the vault's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true while the vault has no derived key.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.key.is_none()
    }

    /// Returns true if the vault file already exists on disk.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Returns true if the vault has never had a passphrase set.
    #[must_use]
    pub fn is_new(&self) -> bool {
        !self.file.entries.contains_key(VERIFIER_ID)
    }

    /// Returns the number of stored secrets, excluding the verifier.
    #[must_use]
    pub fn len(&self) -> usize {
        self.file
            .entries
            .keys()
            .filter(|k| k.as_str() != VERIFIER_ID)
            .count()
    }

    /// Returns true if the vault holds no secrets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the ids of the stored secrets.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.file
            .entries
            .keys()
            .map(String::as_str)
            .filter(|k| *k != VERIFIER_ID)
    }

    /// Derives the key from `passphrase` and verifies it.
    ///
    /// A vault with no verifier yet adopts this passphrase.
    ///
    /// # Errors
    /// Returns [`VaultError::WrongPassphrase`] if the passphrase does not match
    /// an existing vault.
    pub fn unlock(&mut self, passphrase: &str) -> Result<(), VaultError> {
        if passphrase.is_empty() {
            return Err(VaultError::EmptyPassphrase);
        }

        let salt = b64_decode(&self.file.kdf.salt, "salt")?;
        if salt.len() != SALT_LEN {
            return Err(VaultError::Corrupt(format!(
                "salt is {} bytes, expected {SALT_LEN}",
                salt.len()
            )));
        }

        let argon = self.file.kdf.params.to_argon2()?;
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        argon
            .hash_password_into(passphrase.as_bytes(), &salt, key.as_mut())
            .map_err(|e| VaultError::Kdf(e.to_string()))?;

        match self.file.entries.get(VERIFIER_ID) {
            Some(entry) => {
                let plaintext = Self::decrypt_entry(&key, VERIFIER_ID, entry)
                    .map_err(|_| VaultError::WrongPassphrase)?;
                if plaintext.as_slice() != VERIFIER_PLAINTEXT {
                    return Err(VaultError::WrongPassphrase);
                }
                self.key = Some(key);
            }
            None => {
                let entry = Self::encrypt_entry(&key, VERIFIER_ID, VERIFIER_PLAINTEXT)?;
                self.file.entries.insert(VERIFIER_ID.to_string(), entry);
                self.key = Some(key);
                self.dirty = true;
            }
        }

        Ok(())
    }

    /// Drops the derived key, leaving the ciphertext intact.
    pub fn lock(&mut self) {
        self.key = None;
    }

    /// Stores `secret` under `id`, replacing any previous value.
    ///
    /// # Errors
    /// Returns [`VaultError::Locked`] if the vault has not been unlocked.
    pub fn put(&mut self, id: &str, secret: &str) -> Result<(), VaultError> {
        if id == VERIFIER_ID {
            return Err(VaultError::Corrupt("reserved entry id".to_string()));
        }
        let key = self.key.as_ref().ok_or(VaultError::Locked)?;
        let entry = Self::encrypt_entry(key, id, secret.as_bytes())?;
        self.file.entries.insert(id.to_string(), entry);
        self.dirty = true;
        Ok(())
    }

    /// Returns the secret stored under `id`, if any.
    ///
    /// # Errors
    /// Returns [`VaultError::Locked`] if the vault has not been unlocked, or
    /// [`VaultError::Crypto`] if the entry does not authenticate.
    pub fn get(&self, id: &str) -> Result<Option<Zeroizing<String>>, VaultError> {
        if id == VERIFIER_ID {
            return Ok(None);
        }
        let key = self.key.as_ref().ok_or(VaultError::Locked)?;
        let Some(entry) = self.file.entries.get(id) else {
            return Ok(None);
        };

        let plaintext = Self::decrypt_entry(key, id, entry)?;
        let text = String::from_utf8(plaintext)
            .map_err(|_| VaultError::Corrupt(format!("entry {id} is not valid UTF-8")))?;
        Ok(Some(Zeroizing::new(text)))
    }

    /// Removes the secret stored under `id`.
    ///
    /// Returns true if an entry was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        if id == VERIFIER_ID {
            return false;
        }
        let removed = self.file.entries.remove(id).is_some();
        self.dirty |= removed;
        removed
    }

    /// Returns true if there are unsaved changes.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Re-encrypts every entry under a new passphrase.
    ///
    /// # Errors
    /// Returns an error if the vault is locked or the new passphrase is empty.
    pub fn change_passphrase(&mut self, new_passphrase: &str) -> Result<(), VaultError> {
        if new_passphrase.is_empty() {
            return Err(VaultError::EmptyPassphrase);
        }
        let old_key = self.key.as_ref().ok_or(VaultError::Locked)?.clone();

        // Decrypt everything under the old key first, so a failure part way
        // through cannot leave a vault half re-encrypted.
        let mut plaintexts: Vec<(String, Vec<u8>)> = Vec::new();
        for (id, entry) in &self.file.entries {
            if id == VERIFIER_ID {
                continue;
            }
            plaintexts.push((id.clone(), Self::decrypt_entry(&old_key, id, entry)?));
        }

        let mut salt = [0u8; SALT_LEN];
        random_bytes(&mut salt)?;
        self.file.kdf.salt = b64_encode(&salt);

        let argon = self.file.kdf.params.to_argon2()?;
        let mut new_key = Zeroizing::new([0u8; KEY_LEN]);
        argon
            .hash_password_into(new_passphrase.as_bytes(), &salt, new_key.as_mut())
            .map_err(|e| VaultError::Kdf(e.to_string()))?;

        let mut entries = BTreeMap::new();
        entries.insert(
            VERIFIER_ID.to_string(),
            Self::encrypt_entry(&new_key, VERIFIER_ID, VERIFIER_PLAINTEXT)?,
        );
        for (id, plaintext) in plaintexts {
            let entry = Self::encrypt_entry(&new_key, &id, &plaintext)?;
            entries.insert(id, entry);
        }

        self.file.entries = entries;
        self.key = Some(new_key);
        self.dirty = true;
        Ok(())
    }

    /// Writes the vault to disk.
    ///
    /// The file is written to a temporary path and renamed, so an interrupted
    /// save cannot truncate an existing vault. On Unix the file is chmod 0600.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn save(&mut self) -> Result<(), VaultError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let text = toml::to_string_pretty(&self.file)?;
        let temp = self.path.with_extension("vault.tmp");
        fs::write(&temp, text.as_bytes())?;
        restrict_permissions(&temp)?;
        fs::rename(&temp, &self.path)?;
        restrict_permissions(&self.path)?;

        self.dirty = false;
        Ok(())
    }

    fn cipher(key: &[u8; KEY_LEN]) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(key.into())
    }

    fn encrypt_entry(
        key: &[u8; KEY_LEN],
        id: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedEntry, VaultError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        random_bytes(&mut nonce_bytes)?;
        let nonce = XNonce::from(nonce_bytes);

        let ciphertext = Self::cipher(key)
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Crypto)?;

        Ok(EncryptedEntry {
            nonce: b64_encode(&nonce_bytes),
            ciphertext: b64_encode(&ciphertext),
        })
    }

    fn decrypt_entry(
        key: &[u8; KEY_LEN],
        id: &str,
        entry: &EncryptedEntry,
    ) -> Result<Vec<u8>, VaultError> {
        let nonce_bytes = b64_decode(&entry.nonce, "nonce")?;
        let nonce_array: [u8; NONCE_LEN] = nonce_bytes.as_slice().try_into().map_err(|_| {
            VaultError::Corrupt(format!(
                "nonce is {} bytes, expected {NONCE_LEN}",
                nonce_bytes.len()
            ))
        })?;
        let ciphertext = b64_decode(&entry.ciphertext, "ciphertext")?;

        let nonce = XNonce::from(nonce_array);
        Self::cipher(key)
            .decrypt(
                &nonce,
                Payload {
                    msg: &ciphertext,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Crypto)
    }
}

/// Restricts a file to its owner where the platform supports it.
///
/// On Unix this is chmod 0600. On Windows a file created under the user's
/// profile already inherits a user-scoped ACL, and there is no portable way to
/// tighten it further from std; the IPC endpoint is protected separately.
pub(crate) fn restrict_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("secrets.vault");
        let vault = Vault::open(path, KdfParams::fast_insecure()).expect("open");
        (dir, vault)
    }

    #[test]
    fn a_new_vault_is_locked_and_empty() {
        let (_dir, vault) = temp_vault();
        assert!(vault.is_locked());
        assert!(vault.is_new());
        assert!(vault.is_empty());
        assert_eq!(vault.len(), 0);
    }

    #[test]
    fn unlocking_a_new_vault_adopts_the_passphrase() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("correct horse").expect("unlock");
        assert!(!vault.is_locked());
        assert!(!vault.is_new());
    }

    #[test]
    fn secrets_round_trip_through_disk() {
        let (dir, mut vault) = temp_vault();
        vault.unlock("passphrase").expect("unlock");
        vault.put("ssh/1/password", "hunter2").expect("put");
        vault.save().expect("save");

        let mut reopened =
            Vault::open(dir.path().join("secrets.vault"), KdfParams::fast_insecure())
                .expect("reopen");
        reopened.unlock("passphrase").expect("unlock");
        assert_eq!(
            reopened
                .get("ssh/1/password")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("hunter2")
        );
    }

    #[test]
    fn the_wrong_passphrase_is_reported_as_such() {
        let (dir, mut vault) = temp_vault();
        vault.unlock("right").expect("unlock");
        vault.put("k", "v").expect("put");
        vault.save().expect("save");

        let mut reopened =
            Vault::open(dir.path().join("secrets.vault"), KdfParams::fast_insecure())
                .expect("reopen");
        match reopened.unlock("wrong") {
            Err(VaultError::WrongPassphrase) => {}
            other => panic!("expected WrongPassphrase, got {other:?}"),
        }
        assert!(reopened.is_locked());
    }

    #[test]
    fn an_empty_passphrase_is_refused() {
        let (_dir, mut vault) = temp_vault();
        match vault.unlock("") {
            Err(VaultError::EmptyPassphrase) => {}
            other => panic!("expected EmptyPassphrase, got {other:?}"),
        }
    }

    #[test]
    fn a_locked_vault_refuses_reads_and_writes() {
        let (_dir, mut vault) = temp_vault();
        assert!(matches!(vault.put("k", "v"), Err(VaultError::Locked)));
        assert!(matches!(vault.get("k"), Err(VaultError::Locked)));
    }

    #[test]
    fn locking_forgets_the_key_but_keeps_the_ciphertext() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("k", "v").expect("put");
        vault.lock();
        assert!(vault.is_locked());
        assert_eq!(vault.len(), 1);
        vault.unlock("p").expect("re-unlock");
        assert_eq!(
            vault.get("k").expect("get").as_deref().map(String::as_str),
            Some("v")
        );
    }

    #[test]
    fn missing_entries_return_none() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        assert!(vault.get("absent").expect("get").is_none());
    }

    #[test]
    fn removing_an_entry_drops_it() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("k", "v").expect("put");
        assert!(vault.remove("k"));
        assert!(!vault.remove("k"));
        assert!(vault.get("k").expect("get").is_none());
    }

    #[test]
    fn the_plaintext_never_appears_in_the_file() {
        let (dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault
            .put("ssh/1/password", "super-secret-value")
            .expect("put");
        vault.save().expect("save");

        let raw = std::fs::read_to_string(dir.path().join("secrets.vault")).expect("read");
        assert!(
            !raw.contains("super-secret-value"),
            "the vault file must not contain the plaintext"
        );
        assert!(raw.contains("argon2id"));
    }

    #[test]
    fn each_save_uses_a_fresh_nonce() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("k", "same-value").expect("first");
        let first = vault.file.entries.get("k").expect("entry").clone();
        vault.put("k", "same-value").expect("second");
        let second = vault.file.entries.get("k").expect("entry").clone();

        assert_ne!(
            first.nonce, second.nonce,
            "re-encrypting the same value must not reuse a nonce"
        );
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn an_entry_cannot_be_moved_to_another_id() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("a", "value-a").expect("put a");
        vault.put("b", "value-b").expect("put b");

        let stolen = vault.file.entries.get("a").expect("entry").clone();
        vault.file.entries.insert("b".to_string(), stolen);

        // Authenticated associated data binds the ciphertext to its id.
        assert!(matches!(vault.get("b"), Err(VaultError::Crypto)));
    }

    #[test]
    fn tampering_with_the_ciphertext_is_detected() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("k", "value").expect("put");

        let entry = vault.file.entries.get_mut("k").expect("entry");
        let mut bytes = b64_decode(&entry.ciphertext, "ct").expect("decode");
        bytes[0] ^= 0xFF;
        entry.ciphertext = b64_encode(&bytes);

        assert!(matches!(vault.get("k"), Err(VaultError::Crypto)));
    }

    #[test]
    fn changing_the_passphrase_re_encrypts_every_entry() {
        let (dir, mut vault) = temp_vault();
        vault.unlock("old").expect("unlock");
        vault.put("a", "one").expect("put a");
        vault.put("b", "two").expect("put b");
        vault.change_passphrase("new").expect("rekey");
        vault.save().expect("save");

        let path = dir.path().join("secrets.vault");
        let mut reopened = Vault::open(&path, KdfParams::fast_insecure()).expect("reopen");
        assert!(matches!(
            reopened.unlock("old"),
            Err(VaultError::WrongPassphrase)
        ));

        let mut reopened = Vault::open(&path, KdfParams::fast_insecure()).expect("reopen");
        reopened.unlock("new").expect("unlock with new passphrase");
        assert_eq!(
            reopened
                .get("a")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("one")
        );
        assert_eq!(
            reopened
                .get("b")
                .expect("get")
                .as_deref()
                .map(String::as_str),
            Some("two")
        );
    }

    #[test]
    fn changing_the_passphrase_needs_an_unlocked_vault() {
        let (_dir, mut vault) = temp_vault();
        assert!(matches!(
            vault.change_passphrase("new"),
            Err(VaultError::Locked)
        ));
    }

    #[test]
    fn ids_exclude_the_internal_verifier() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("one", "1").expect("put");
        vault.put("two", "2").expect("put");
        let ids: Vec<&str> = vault.ids().collect();
        assert_eq!(ids, vec!["one", "two"]);
    }

    #[test]
    fn the_verifier_id_is_reserved() {
        let (_dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        assert!(vault.put(VERIFIER_ID, "x").is_err());
        assert!(vault.get(VERIFIER_ID).expect("get").is_none());
        assert!(!vault.remove(VERIFIER_ID));
    }

    #[test]
    fn a_damaged_file_is_rejected_rather_than_guessed_at() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("secrets.vault");
        std::fs::write(&path, "this is not toml {{{").expect("write");
        assert!(matches!(
            Vault::open(&path, KdfParams::fast_insecure()),
            Err(VaultError::Parse(_))
        ));
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("secrets.vault");
        std::fs::write(
            &path,
            "version = 99\n[kdf]\nalgorithm = \"argon2id\"\nsalt = \"AA==\"\nmemory_kib = 64\niterations = 1\nparallelism = 1\n",
        )
        .expect("write");
        assert!(matches!(
            Vault::open(&path, KdfParams::fast_insecure()),
            Err(VaultError::Corrupt(_))
        ));
    }

    #[test]
    fn a_short_salt_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("secrets.vault");
        std::fs::write(
            &path,
            "version = 1\n[kdf]\nalgorithm = \"argon2id\"\nsalt = \"AA==\"\nmemory_kib = 64\niterations = 1\nparallelism = 1\n",
        )
        .expect("write");
        let mut vault = Vault::open(&path, KdfParams::fast_insecure()).expect("open");
        assert!(matches!(vault.unlock("p"), Err(VaultError::Corrupt(_))));
    }

    #[test]
    fn saving_is_atomic_and_leaves_no_temp_file() {
        let (dir, mut vault) = temp_vault();
        vault.unlock("p").expect("unlock");
        vault.put("k", "v").expect("put");
        vault.save().expect("save");
        assert!(!vault.is_dirty());

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "left behind {leftovers:?}");
    }

    #[test]
    fn interactive_parameters_meet_the_owasp_floor() {
        let p = KdfParams::interactive();
        assert!(p.memory_kib >= 19_456);
        assert!(p.iterations >= 2);
        assert!(p.parallelism >= 1);
        assert!(p.to_argon2().is_ok());
    }

    #[test]
    fn interactive_parameters_produce_a_usable_vault() {
        // Guards against a params combination the KDF rejects at runtime.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("secrets.vault");
        let mut vault = Vault::open(&path, KdfParams::interactive()).expect("open");
        vault.unlock("a real passphrase").expect("unlock");
        vault.put("k", "v").expect("put");
        assert_eq!(
            vault.get("k").expect("get").as_deref().map(String::as_str),
            Some("v")
        );
    }
}
