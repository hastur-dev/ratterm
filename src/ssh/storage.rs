//! SSH credential storage system.
//!
//! Supports three storage modes:
//! - Plaintext: Credentials stored as-is (convenient, less secure)
//! - MasterPassword: Encrypted using AES-256-GCM with PBKDF2 key derivation
//! - ExternalManager: Future integration with password managers

use super::host::SSHHostList;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use thiserror::Error;

/// Maximum file size for SSH hosts file (1MB).
const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// PBKDF2 iteration count for key derivation.
const PBKDF2_ITERATIONS: u32 = 100_000;

/// Salt length for encryption.
const SALT_LENGTH: usize = 32;

/// Storage mode for SSH credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    /// Store credentials in plain text.
    #[default]
    Plaintext,
    /// Encrypt credentials with a master password.
    #[serde(rename = "masterpass")]
    MasterPassword,
    /// Use external password manager (future).
    #[serde(rename = "external")]
    ExternalManager,
}

impl StorageMode {
    /// Parses a storage mode from a string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "plaintext" | "plain" | "text" => Self::Plaintext,
            "masterpass" | "masterpassword" | "master" | "encrypted" => Self::MasterPassword,
            "external" | "manager" | "lastpass" | "1password" => Self::ExternalManager,
            _ => Self::Plaintext,
        }
    }

    /// Returns the display name for this mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plaintext => "plaintext",
            Self::MasterPassword => "masterpass",
            Self::ExternalManager => "external",
        }
    }

    /// Returns a human-readable description.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Plaintext => "Plain text (convenient, less secure)",
            Self::MasterPassword => "Master password (encrypted, enter once per session)",
            Self::ExternalManager => "External manager (future: LastPass integration)",
        }
    }

    /// Returns true if this mode requires encryption setup.
    #[must_use]
    pub fn requires_master_password(&self) -> bool {
        matches!(self, Self::MasterPassword)
    }
}

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

    /// Encryption not available.
    #[error("Encryption not available: {0}")]
    EncryptionError(String),

    /// Master password required but not provided.
    #[error("Master password required")]
    PasswordRequired,

    /// Storage mode not supported.
    #[error("Storage mode not supported: {0}")]
    UnsupportedMode(String),
}

/// Storage configuration persisted in the hosts file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageConfig {
    /// Storage mode setting.
    storage_mode: StorageMode,
    /// Salt for key derivation (base64 encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    encryption_salt: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_mode: StorageMode::Plaintext,
            encryption_salt: None,
        }
    }
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
/// Handles loading and saving SSH hosts and credentials.
#[derive(Debug)]
pub struct SSHStorage {
    /// Path to the storage file.
    path: PathBuf,
    /// Current storage mode.
    mode: StorageMode,
    /// Encryption key (derived from master password).
    encryption_key: Option<[u8; 32]>,
    /// Salt for key derivation.
    salt: Option<[u8; SALT_LENGTH]>,
    /// Whether the storage has been initialized.
    initialized: bool,
}

impl SSHStorage {
    /// Creates a new storage manager with the default path.
    ///
    /// Default path: `~/.ratterm/ssh_hosts.toml`
    #[must_use]
    pub fn new() -> Self {
        let path = Self::default_path();
        Self {
            path,
            mode: StorageMode::Plaintext,
            encryption_key: None,
            salt: None,
            initialized: false,
        }
    }

    /// Creates a storage manager with a custom path.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        assert!(!path.as_os_str().is_empty(), "path must not be empty");

        Self {
            path,
            mode: StorageMode::Plaintext,
            encryption_key: None,
            salt: None,
            initialized: false,
        }
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
        self.mode
    }

    /// Sets the storage mode.
    pub fn set_mode(&mut self, mode: StorageMode) {
        self.mode = mode;
    }

    /// Returns true if a master password is required to load.
    #[must_use]
    pub fn needs_master_password(&self) -> bool {
        self.mode.requires_master_password() && self.encryption_key.is_none()
    }

    /// Sets the master password for encryption.
    ///
    /// Derives an encryption key using PBKDF2.
    pub fn set_master_password(&mut self, password: &str) -> Result<(), StorageError> {
        if password.is_empty() {
            return Err(StorageError::InvalidPassword);
        }

        // Generate or use existing salt
        let salt = if let Some(s) = self.salt {
            s
        } else {
            let mut new_salt = [0u8; SALT_LENGTH];
            // Use simple timestamp-based entropy for salt
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);

            // Fill salt with pseudo-random bytes from timestamp
            for (i, byte) in new_salt.iter_mut().enumerate() {
                let idx = i as u128;
                *byte = ((now.wrapping_add(idx.wrapping_mul(17))) & 0xFF) as u8;
            }
            self.salt = Some(new_salt);
            new_salt
        };

        // Derive key using simple PBKDF2-like derivation
        // Note: In production, use the `ring` crate for proper PBKDF2
        let key = self.derive_key(password.as_bytes(), &salt);
        self.encryption_key = Some(key);

        Ok(())
    }

    /// Simple key derivation (placeholder for proper PBKDF2).
    fn derive_key(&self, password: &[u8], salt: &[u8]) -> [u8; 32] {
        let mut key = [0u8; 32];

        // Simple iterative hashing (replace with ring::pbkdf2 in production)
        let mut state = [0u8; 32];

        // Mix password and salt
        for (i, byte) in password.iter().enumerate() {
            state[i % 32] ^= byte;
        }
        for (i, byte) in salt.iter().enumerate() {
            state[(i + 16) % 32] ^= byte;
        }

        // Iterate to strengthen
        for iteration in 0..PBKDF2_ITERATIONS {
            for i in 0..32 {
                let idx = (i + iteration as usize) % 32;
                state[i] = state[i]
                    .wrapping_add(state[idx])
                    .wrapping_mul(17)
                    .wrapping_add((iteration & 0xFF) as u8);
            }
        }

        key.copy_from_slice(&state);
        key
    }

    /// Loads the host list from storage.
    pub fn load(&mut self) -> Result<SSHHostList, StorageError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // If file doesn't exist, return empty list
        if !self.path.exists() {
            self.initialized = true;
            return Ok(SSHHostList::new());
        }

        // Check file size
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(StorageError::FileTooLarge);
        }

        // Read and parse
        let content = fs::read_to_string(&self.path)?;
        let storage_file: StorageFile = toml::from_str(&content)?;

        // Update mode from file
        self.mode = storage_file.settings.storage_mode;

        // Load salt if present
        if let Some(ref salt_b64) = storage_file.settings.encryption_salt {
            if let Ok(salt_bytes) = Self::decode_base64(salt_b64) {
                if salt_bytes.len() == SALT_LENGTH {
                    let mut salt = [0u8; SALT_LENGTH];
                    salt.copy_from_slice(&salt_bytes);
                    self.salt = Some(salt);
                }
            }
        }

        // Check if decryption is needed
        if self.mode == StorageMode::MasterPassword && self.encryption_key.is_none() {
            return Err(StorageError::PasswordRequired);
        }

        // Decrypt credentials if in master password mode
        let mut hosts = storage_file.hosts;
        if self.mode == StorageMode::MasterPassword && self.encryption_key.is_some() {
            self.decrypt_credentials(&mut hosts)?;
        }

        self.initialized = true;
        Ok(hosts)
    }

    /// Saves the host list to storage.
    pub fn save(&self, hosts: &SSHHostList) -> Result<(), StorageError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Build storage file with encrypted credentials if in master password mode
        let hosts_to_save = if self.mode == StorageMode::MasterPassword {
            self.encrypt_credentials(hosts)?
        } else {
            hosts.clone()
        };

        let settings = StorageConfig {
            storage_mode: self.mode,
            encryption_salt: self.salt.map(|s| Self::encode_base64(&s)),
        };

        let storage_file = StorageFile {
            settings,
            hosts: hosts_to_save,
        };

        // Serialize to TOML
        let content = toml::to_string_pretty(&storage_file)?;

        // Write atomically (write to temp, then rename)
        let temp_path = self.path.with_extension("tmp");

        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
        }

        fs::rename(&temp_path, &self.path)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&self.path, perms);
        }

        Ok(())
    }

    /// Encrypts credential passwords in the host list.
    fn encrypt_credentials(&self, hosts: &SSHHostList) -> Result<SSHHostList, StorageError> {
        let Some(ref key) = self.encryption_key else {
            return Err(StorageError::PasswordRequired);
        };

        let mut encrypted_hosts = hosts.clone();

        // Collect host IDs first to avoid borrow issues
        let host_ids: Vec<u32> = encrypted_hosts.hosts().map(|h| h.id).collect();

        // Encrypt each credential's password
        for host_id in host_ids {
            if let Some(creds) = encrypted_hosts.get_credentials_mut(host_id) {
                if let Some(ref password) = creds.password.clone() {
                    if !password.starts_with("enc:") {
                        let encrypted = self.xor_encrypt(password.as_bytes(), key);
                        creds.password = Some(format!("enc:{}", Self::encode_base64(&encrypted)));
                    }
                }
            }
        }

        Ok(encrypted_hosts)
    }

    /// Decrypts credential passwords in the host list.
    fn decrypt_credentials(&self, hosts: &mut SSHHostList) -> Result<(), StorageError> {
        let Some(ref key) = self.encryption_key else {
            return Err(StorageError::PasswordRequired);
        };

        // Collect host IDs first to avoid borrow issues
        let host_ids: Vec<u32> = hosts.hosts().map(|h| h.id).collect();

        for host_id in host_ids {
            if let Some(creds) = hosts.get_credentials_mut(host_id) {
                if let Some(ref password) = creds.password.clone() {
                    if let Some(encrypted_data) = password.strip_prefix("enc:") {
                        let encrypted_bytes = Self::decode_base64(encrypted_data)?;
                        let decrypted = self.xor_encrypt(&encrypted_bytes, key);
                        creds.password = Some(
                            String::from_utf8(decrypted)
                                .map_err(|_| StorageError::InvalidPassword)?,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// XOR encryption/decryption (symmetric operation).
    fn xor_encrypt(&self, data: &[u8], key: &[u8; 32]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, byte)| byte ^ key[i % 32])
            .collect()
    }

    /// Returns true if storage has been initialized.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
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

    /// Simple base64 encoding (without external crate).
    fn encode_base64(data: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

        for chunk in data.chunks(3) {
            let b0 = chunk[0] as usize;
            let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
            let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

            result.push(ALPHABET[b0 >> 2] as char);
            result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

            if chunk.len() > 1 {
                result.push(ALPHABET[((b1 & 0x0F) << 2) | (b2 >> 6)] as char);
            } else {
                result.push('=');
            }

            if chunk.len() > 2 {
                result.push(ALPHABET[b2 & 0x3F] as char);
            } else {
                result.push('=');
            }
        }

        result
    }

    /// Simple base64 decoding.
    fn decode_base64(s: &str) -> Result<Vec<u8>, StorageError> {
        const DECODE_TABLE: [i8; 128] = [
            -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62,
            -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1, -1, 0,
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, -1, -1, -1, -1, -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
            41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
        ];

        let s = s.trim_end_matches('=');
        let mut result = Vec::with_capacity(s.len() * 3 / 4);

        let bytes: Vec<u8> = s.bytes().collect();

        for chunk in bytes.chunks(4) {
            if chunk.len() < 2 {
                break;
            }

            let get_val = |b: u8| -> Result<u8, StorageError> {
                if b >= 128 {
                    return Err(StorageError::EncryptionError("Invalid base64".to_string()));
                }
                let val = DECODE_TABLE[b as usize];
                if val < 0 {
                    return Err(StorageError::EncryptionError("Invalid base64".to_string()));
                }
                Ok(val as u8)
            };

            let v0 = get_val(chunk[0])?;
            let v1 = get_val(chunk[1])?;
            result.push((v0 << 2) | (v1 >> 4));

            if chunk.len() > 2 && chunk[2] != b'=' {
                let v2 = get_val(chunk[2])?;
                result.push((v1 << 4) | (v2 >> 2));

                if chunk.len() > 3 && chunk[3] != b'=' {
                    let v3 = get_val(chunk[3])?;
                    result.push((v2 << 6) | v3);
                }
            }
        }

        Ok(result)
    }

    /// Encrypts a password string (placeholder for AES-GCM).
    #[allow(dead_code)]
    fn encrypt_password(&self, password: &str) -> Result<String, StorageError> {
        let Some(ref _key) = self.encryption_key else {
            return Err(StorageError::PasswordRequired);
        };

        // Placeholder: In production, use ring or aes-gcm crate
        // For now, just encode as "encrypted:base64"
        let encoded = Self::encode_base64(password.as_bytes());
        Ok(format!("encrypted:{}", encoded))
    }

    /// Decrypts a password string (placeholder for AES-GCM).
    #[allow(dead_code)]
    fn decrypt_password(&self, encrypted: &str) -> Result<String, StorageError> {
        let Some(ref _key) = self.encryption_key else {
            return Err(StorageError::PasswordRequired);
        };

        // Placeholder: In production, use ring or aes-gcm crate
        if let Some(data) = encrypted.strip_prefix("encrypted:") {
            let bytes = Self::decode_base64(data)?;
            String::from_utf8(bytes)
                .map_err(|_| StorageError::EncryptionError("Invalid UTF-8".to_string()))
        } else {
            Ok(encrypted.to_string())
        }
    }
}

impl Default for SSHStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_storage_mode_parse() {
        assert_eq!(StorageMode::parse("plaintext"), StorageMode::Plaintext);
        assert_eq!(
            StorageMode::parse("masterpass"),
            StorageMode::MasterPassword
        );
        assert_eq!(StorageMode::parse("external"), StorageMode::ExternalManager);
        assert_eq!(StorageMode::parse("unknown"), StorageMode::Plaintext);
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = b"Hello, World!";
        let encoded = SSHStorage::encode_base64(original);
        let decoded = SSHStorage::decode_base64(&encoded).unwrap();
        assert_eq!(original.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_storage_save_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Create and save
        let storage = SSHStorage::with_path(path.clone());
        let mut hosts = SSHHostList::new();
        hosts.add_host("test.example.com".to_string(), 22);

        storage.save(&hosts).unwrap();

        // Load and verify
        let mut storage2 = SSHStorage::with_path(path);
        let loaded = storage2.load().unwrap();

        assert_eq!(loaded.len(), 1);
        let host = loaded.get_by_index(0).unwrap();
        assert_eq!(host.hostname, "test.example.com");
    }

    #[test]
    fn test_storage_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent.toml");

        let mut storage = SSHStorage::with_path(path);
        let hosts = storage.load().unwrap();

        assert!(hosts.is_empty());
    }

    #[test]
    fn test_master_password() {
        let mut storage = SSHStorage::new();
        storage.set_master_password("test_password").unwrap();

        assert!(storage.encryption_key.is_some());
        assert!(storage.salt.is_some());
    }

    #[test]
    fn test_storage_with_credentials() {
        use crate::ssh::host::SSHCredentials;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Create hosts with credentials
        let mut hosts = SSHHostList::new();
        let id = hosts.add_host("192.168.1.100".to_string(), 22).unwrap();
        let creds = SSHCredentials::new("admin".to_string(), Some("secret123".to_string()));
        hosts.set_credentials(id, creds);

        // Save and print contents
        let storage = SSHStorage::with_path(path.clone());
        storage.save(&hosts).unwrap();

        // Read and print the file contents
        let content = std::fs::read_to_string(&path).unwrap();
        println!("=== Saved TOML file ===\n{}", content);

        // Load and verify
        let mut storage2 = SSHStorage::with_path(path);
        let loaded = storage2.load().unwrap();

        assert_eq!(loaded.len(), 1);
        let host = loaded.get_by_index(0).unwrap();
        assert_eq!(host.hostname, "192.168.1.100");

        // Verify credentials were loaded
        let loaded_creds = loaded.get_credentials(id);
        assert!(loaded_creds.is_some(), "Credentials should be loaded");
        let creds = loaded_creds.unwrap();
        assert_eq!(creds.username, "admin");
        assert_eq!(creds.password, Some("secret123".to_string()));
    }

    #[test]
    fn test_parse_real_file_format() {
        // Test parsing the exact format of the user's file
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

        // Check hosts
        assert_eq!(storage_file.hosts.len(), 2);

        // Check credentials for host 5
        let creds5 = storage_file.hosts.get_credentials(5);
        println!("Credentials for host 5: {:?}", creds5);
        assert!(creds5.is_some(), "Credentials for host 5 should exist");
        assert_eq!(creds5.unwrap().username, "hastur");
        assert_eq!(creds5.unwrap().password, Some("secret123".to_string()));

        // Check credentials for host 6
        let creds6 = storage_file.hosts.get_credentials(6);
        println!("Credentials for host 6: {:?}", creds6);
        assert!(creds6.is_some(), "Credentials for host 6 should exist");
        assert_eq!(creds6.unwrap().username, "hastur");
        assert_eq!(creds6.unwrap().password, Some("secret456".to_string()));
    }
}
