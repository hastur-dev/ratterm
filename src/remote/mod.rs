//! Remote file operations module.
//!
//! Provides SFTP-based remote file management for SSH terminals,
//! including fetching, caching, and saving remote files.

pub mod browser;
pub mod sftp;

pub use browser::{RemoteFileBrowser, RemoteFileEntry};
pub use sftp::{RemoteDirEntry, SftpClient, SftpError};

use std::collections::HashMap;
use std::path::PathBuf;

use crate::terminal::SSHContext;

/// Represents a remote file being edited locally.
#[derive(Debug, Clone)]
pub struct RemoteFile {
    /// SSH context for the connection.
    pub ssh_context: SSHContext,
    /// Remote file path.
    pub remote_path: String,
    /// Local cache path.
    pub local_cache_path: PathBuf,
    /// Remote working directory when file was opened.
    pub remote_cwd: String,
    /// Remote modification time (unix timestamp).
    pub remote_mtime: Option<u64>,
}

impl RemoteFile {
    /// Creates a new remote file reference.
    #[must_use]
    pub fn new(
        ssh_context: SSHContext,
        remote_path: String,
        local_cache_path: PathBuf,
        remote_cwd: String,
    ) -> Self {
        assert!(!remote_path.is_empty(), "remote_path must not be empty");

        Self {
            ssh_context,
            remote_path,
            local_cache_path,
            remote_cwd,
            remote_mtime: None,
        }
    }

    /// Returns a display string for the remote file.
    #[must_use]
    pub fn display_string(&self) -> String {
        format!(
            "[SSH] {}:{}",
            self.ssh_context.display_string(),
            self.remote_path
        )
    }

    /// Returns the filename portion of the remote path.
    /// Uses POSIX-style path parsing (forward slashes) regardless of local OS.
    #[must_use]
    pub fn filename(&self) -> &str {
        // Remote paths are always POSIX-style, so split on forward slash
        self.remote_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.remote_path)
    }
}

/// Error type for remote file operations.
#[derive(Debug)]
pub enum RemoteError {
    /// SFTP error.
    Sftp(SftpError),
    /// Local I/O error.
    Io(std::io::Error),
    /// No connection available.
    NotConnected,
    /// File not found.
    FileNotFound(String),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sftp(e) => write!(f, "{}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::NotConnected => write!(f, "Not connected to remote host"),
            Self::FileNotFound(path) => write!(f, "Remote file not found: {}", path),
        }
    }
}

impl std::error::Error for RemoteError {}

impl From<SftpError> for RemoteError {
    fn from(e: SftpError) -> Self {
        Self::Sftp(e)
    }
}

impl From<std::io::Error> for RemoteError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Manages remote file operations and connection pooling.
pub struct RemoteFileManager {
    /// Cache directory for remote files.
    cache_dir: PathBuf,
    /// Cached SFTP clients by host key (user@host:port).
    clients: HashMap<String, SftpClient>,
    /// Currently cached remote files by local cache path.
    cached_files: HashMap<PathBuf, RemoteFile>,
}

impl RemoteFileManager {
    /// Creates a new remote file manager.
    ///
    /// Uses the system temp directory for caching.
    #[must_use]
    pub fn new() -> Self {
        let cache_dir = std::env::temp_dir().join("ratterm_remote");
        Self {
            cache_dir,
            clients: HashMap::new(),
            cached_files: HashMap::new(),
        }
    }

    /// Creates a unique key for an SSH context.
    fn context_key(ctx: &SSHContext) -> String {
        format!("{}@{}:{}", ctx.username, ctx.hostname, ctx.port)
    }

    /// Generates a unique cache filename for a remote file.
    /// Uses POSIX-style path parsing for remote paths regardless of local OS.
    fn cache_filename(&self, ctx: &SSHContext, remote_path: &str) -> PathBuf {
        // Use hash to create unique filename
        let hash = {
            let input = format!("{}:{}", Self::context_key(ctx), remote_path);
            let mut h: u64 = 0;
            for byte in input.bytes() {
                h = h.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
            h
        };

        // Remote paths are always POSIX-style, so split on forward slash
        let filename = remote_path.rsplit('/').next().unwrap_or("file");

        self.cache_dir.join(format!("{}_{}", hash, filename))
    }

    /// Gets or creates an SFTP client for the given context.
    ///
    /// # Errors
    /// Returns error if connection fails.
    pub fn get_client(&mut self, ctx: &SSHContext) -> Result<&SftpClient, RemoteError> {
        let key = Self::context_key(ctx);

        // Check if we need a new connection
        let need_new = match self.clients.get(&key) {
            Some(client) => !client.is_connected(),
            None => true,
        };

        if need_new {
            // Create new connection
            let client = SftpClient::connect(ctx)?;
            self.clients.insert(key.clone(), client);
        }

        self.clients.get(&key).ok_or(RemoteError::NotConnected)
    }

    /// Gets the current working directory on the remote host.
    ///
    /// # Errors
    /// Returns error if connection or command fails.
    pub fn get_remote_cwd(&mut self, ctx: &SSHContext) -> Result<String, RemoteError> {
        let client = self.get_client(ctx)?;
        Ok(client.get_cwd()?)
    }

    /// Lists the contents of a directory on the remote host.
    ///
    /// # Errors
    /// Returns error if connection or listing fails.
    pub fn list_dir(
        &mut self,
        ctx: &SSHContext,
        path: &str,
    ) -> Result<Vec<RemoteDirEntry>, RemoteError> {
        assert!(!path.is_empty(), "path must not be empty");

        let client = self.get_client(ctx)?;
        Ok(client.list_dir(path)?)
    }

    /// Resolves a potentially relative path against the remote CWD.
    #[must_use]
    pub fn resolve_path(path: &str, cwd: &str) -> String {
        assert!(!path.is_empty(), "path must not be empty");

        if path.starts_with('/') {
            // Absolute path
            path.to_string()
        } else if path.starts_with("~/") {
            // Home-relative path - keep as-is for shell expansion
            path.to_string()
        } else {
            // Relative path - join with CWD
            format!("{}/{}", cwd.trim_end_matches('/'), path)
        }
    }

    /// Fetches a remote file and caches it locally.
    ///
    /// # Returns
    /// A tuple of (file content, RemoteFile metadata).
    ///
    /// # Errors
    /// Returns error if fetch fails.
    pub fn fetch_file(
        &mut self,
        ctx: &SSHContext,
        remote_path: &str,
        cwd: &str,
    ) -> Result<(String, RemoteFile), RemoteError> {
        assert!(!remote_path.is_empty(), "remote_path must not be empty");

        // Resolve the path
        let resolved_path = Self::resolve_path(remote_path, cwd);

        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir)?;

        // Get or create client
        let client = self.get_client(ctx)?;

        // Read the remote file
        let (content, mtime) = client.read_file(&resolved_path).map_err(|e| {
            if matches!(e, SftpError::FileError(_)) {
                RemoteError::FileNotFound(resolved_path.clone())
            } else {
                RemoteError::Sftp(e)
            }
        })?;

        // Cache locally
        let cache_path = self.cache_filename(ctx, &resolved_path);
        std::fs::write(&cache_path, &content)?;

        // Create remote file metadata
        let mut remote_file = RemoteFile::new(
            ctx.clone(),
            resolved_path,
            cache_path.clone(),
            cwd.to_string(),
        );
        remote_file.remote_mtime = mtime;

        // Store in cache map
        self.cached_files.insert(cache_path, remote_file.clone());

        Ok((content, remote_file))
    }

    /// Saves content to a remote file.
    ///
    /// # Errors
    /// Returns error if save fails.
    pub fn save_file(
        &mut self,
        remote_file: &RemoteFile,
        content: &str,
    ) -> Result<(), RemoteError> {
        // Get client
        let client = self.get_client(&remote_file.ssh_context)?;

        // Write to remote
        client.write_file(&remote_file.remote_path, content)?;

        // Update local cache
        std::fs::write(&remote_file.local_cache_path, content)?;

        Ok(())
    }

    /// Checks if a local path is a cached remote file.
    #[must_use]
    pub fn get_remote_file(&self, local_path: &PathBuf) -> Option<&RemoteFile> {
        self.cached_files.get(local_path)
    }

    /// Removes a file from the cache.
    pub fn remove_from_cache(&mut self, local_path: &PathBuf) {
        if let Some(remote_file) = self.cached_files.remove(local_path) {
            // Try to delete the local cache file
            let _ = std::fs::remove_file(&remote_file.local_cache_path);
        }
    }

    /// Cleans up old cache files.
    pub fn cleanup_cache(&mut self) {
        // Remove all local cache files
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        self.cached_files.clear();
    }

    /// Disconnects all SFTP clients.
    pub fn disconnect_all(&mut self) {
        self.clients.clear();
    }
}

impl Default for RemoteFileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RemoteFileManager {
    fn drop(&mut self) {
        self.cleanup_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_file_display() {
        let ctx = SSHContext::new("user".to_string(), "host.com".to_string(), 22);
        let rf = RemoteFile::new(
            ctx,
            "/home/user/file.txt".to_string(),
            PathBuf::from("/tmp/cache"),
            "/home/user".to_string(),
        );

        assert!(rf.display_string().contains("[SSH]"));
        assert!(rf.display_string().contains("user@host.com"));
        assert!(rf.display_string().contains("/home/user/file.txt"));
    }

    #[test]
    fn test_remote_file_filename() {
        let ctx = SSHContext::new("user".to_string(), "host.com".to_string(), 22);
        let rf = RemoteFile::new(
            ctx,
            "/path/to/myfile.rs".to_string(),
            PathBuf::from("/tmp/cache"),
            "/path/to".to_string(),
        );

        assert_eq!(rf.filename(), "myfile.rs");
    }

    #[test]
    fn test_resolve_path() {
        // Absolute path
        assert_eq!(
            RemoteFileManager::resolve_path("/absolute/path.txt", "/home/user"),
            "/absolute/path.txt"
        );

        // Relative path
        assert_eq!(
            RemoteFileManager::resolve_path("relative/file.txt", "/home/user"),
            "/home/user/relative/file.txt"
        );

        // Home-relative path
        assert_eq!(
            RemoteFileManager::resolve_path("~/documents/file.txt", "/other"),
            "~/documents/file.txt"
        );

        // CWD with trailing slash
        assert_eq!(
            RemoteFileManager::resolve_path("file.txt", "/home/user/"),
            "/home/user/file.txt"
        );
    }

    #[test]
    fn test_context_key() {
        let ctx = SSHContext::new("admin".to_string(), "192.168.1.1".to_string(), 2222);
        let key = RemoteFileManager::context_key(&ctx);
        assert_eq!(key, "admin@192.168.1.1:2222");
    }
}
