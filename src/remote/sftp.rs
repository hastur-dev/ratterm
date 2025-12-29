//! SFTP client for remote file operations.
//!
//! Provides SSH connection and SFTP file transfer functionality
//! using the ssh2 crate.

use ssh2::{Session, Sftp};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use crate::terminal::SSHContext;

/// Maximum file size to read (10MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// SFTP connection timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 10;

/// Read/write timeout in seconds.
const IO_TIMEOUT_SECS: u64 = 30;

/// Represents an entry in a remote directory.
#[derive(Debug, Clone)]
pub struct RemoteDirEntry {
    /// The name of the file or directory.
    pub name: String,
    /// True if this is a directory.
    pub is_directory: bool,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// Error type for SFTP operations.
#[derive(Debug)]
pub enum SftpError {
    /// TCP connection failed.
    ConnectionFailed(String),
    /// SSH handshake failed.
    HandshakeFailed(String),
    /// Authentication failed.
    AuthFailed(String),
    /// SFTP session initialization failed.
    SftpInitFailed(String),
    /// File operation failed.
    FileError(String),
    /// File too large.
    FileTooLarge(u64),
    /// Command execution failed.
    CommandFailed(String),
}

impl std::fmt::Display for SftpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::HandshakeFailed(msg) => write!(f, "SSH handshake failed: {}", msg),
            Self::AuthFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::SftpInitFailed(msg) => write!(f, "SFTP init failed: {}", msg),
            Self::FileError(msg) => write!(f, "File error: {}", msg),
            Self::FileTooLarge(size) => {
                write!(
                    f,
                    "File too large: {} bytes (max: {} bytes)",
                    size, MAX_FILE_SIZE
                )
            }
            Self::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
        }
    }
}

impl std::error::Error for SftpError {}

/// SFTP client for remote file operations.
pub struct SftpClient {
    /// The underlying SSH session.
    session: Session,
    /// The SFTP subsystem.
    sftp: Sftp,
    /// Connection context for reconnection.
    context: SSHContext,
}

impl SftpClient {
    /// Creates a new SFTP client by connecting to the remote host.
    ///
    /// # Errors
    /// Returns error if connection, handshake, or authentication fails.
    pub fn connect(context: &SSHContext) -> Result<Self, SftpError> {
        assert!(!context.hostname.is_empty(), "hostname must not be empty");
        assert!(!context.username.is_empty(), "username must not be empty");
        assert!(context.port > 0, "port must be positive");

        // Establish TCP connection
        let addr = format!("{}:{}", context.hostname, context.port);
        let tcp = TcpStream::connect_timeout(
            &addr
                .parse()
                .map_err(|e| SftpError::ConnectionFailed(format!("{}", e)))?,
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
        )
        .map_err(|e| SftpError::ConnectionFailed(e.to_string()))?;

        // Set read/write timeouts to prevent indefinite hangs
        let io_timeout = Some(std::time::Duration::from_secs(IO_TIMEOUT_SECS));
        let _ = tcp.set_read_timeout(io_timeout);
        let _ = tcp.set_write_timeout(io_timeout);

        // Create SSH session
        let mut session = Session::new().map_err(|e| SftpError::HandshakeFailed(e.to_string()))?;

        // Set session timeout (in milliseconds)
        session.set_timeout(IO_TIMEOUT_SECS as u32 * 1000);

        session.set_tcp_stream(tcp);
        session
            .handshake()
            .map_err(|e| SftpError::HandshakeFailed(e.to_string()))?;

        // Authenticate
        Self::authenticate(&mut session, context)?;

        // Initialize SFTP subsystem
        let sftp = session
            .sftp()
            .map_err(|e| SftpError::SftpInitFailed(e.to_string()))?;

        Ok(Self {
            session,
            sftp,
            context: context.clone(),
        })
    }

    /// Authenticates with the SSH server.
    fn authenticate(session: &mut Session, context: &SSHContext) -> Result<(), SftpError> {
        // Try key-based auth first if available
        if let Some(ref key_path) = context.key_path {
            let key_path = Path::new(key_path);
            if key_path.exists() {
                let result = session.userauth_pubkey_file(
                    &context.username,
                    None, // public key (derive from private)
                    key_path,
                    context.password.as_deref(), // passphrase
                );
                if result.is_ok() && session.authenticated() {
                    return Ok(());
                }
            }
        }

        // Fall back to password auth
        if let Some(ref password) = context.password {
            session
                .userauth_password(&context.username, password)
                .map_err(|e| SftpError::AuthFailed(e.to_string()))?;

            if session.authenticated() {
                return Ok(());
            }
        }

        // Try SSH agent as last resort
        let result = session.userauth_agent(&context.username);
        if result.is_ok() && session.authenticated() {
            return Ok(());
        }

        Err(SftpError::AuthFailed(
            "No valid authentication method".to_string(),
        ))
    }

    /// Reads a file from the remote server.
    ///
    /// # Returns
    /// A tuple of (content as string, modification time as unix timestamp).
    ///
    /// # Errors
    /// Returns error if file cannot be read or is too large.
    pub fn read_file(&self, path: &str) -> Result<(String, Option<u64>), SftpError> {
        assert!(!path.is_empty(), "path must not be empty");

        let path = Path::new(path);

        // Get file stats
        let stat = self
            .sftp
            .stat(path)
            .map_err(|e| SftpError::FileError(format!("Cannot stat file: {}", e)))?;

        // Check file size
        let size = stat.size.unwrap_or(0);
        if size > MAX_FILE_SIZE {
            return Err(SftpError::FileTooLarge(size));
        }

        // Get modification time
        let mtime = stat.mtime;

        // Open and read file
        let mut file = self
            .sftp
            .open(path)
            .map_err(|e| SftpError::FileError(format!("Cannot open file: {}", e)))?;

        let mut content = String::with_capacity(size as usize);
        file.read_to_string(&mut content)
            .map_err(|e| SftpError::FileError(format!("Cannot read file: {}", e)))?;

        Ok((content, mtime))
    }

    /// Writes content to a file on the remote server.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn write_file(&self, path: &str, content: &str) -> Result<(), SftpError> {
        assert!(!path.is_empty(), "path must not be empty");

        let path = Path::new(path);

        // Open file for writing (create or truncate)
        let mut file = self
            .sftp
            .create(path)
            .map_err(|e| SftpError::FileError(format!("Cannot create file: {}", e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| SftpError::FileError(format!("Cannot write file: {}", e)))?;

        Ok(())
    }

    /// Executes a command on the remote server and returns the output.
    ///
    /// # Errors
    /// Returns error if command execution fails.
    pub fn exec_command(&self, command: &str) -> Result<String, SftpError> {
        assert!(!command.is_empty(), "command must not be empty");

        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| SftpError::CommandFailed(format!("Cannot open channel: {}", e)))?;

        channel
            .exec(command)
            .map_err(|e| SftpError::CommandFailed(format!("Cannot execute command: {}", e)))?;

        let mut output = String::new();
        channel
            .read_to_string(&mut output)
            .map_err(|e| SftpError::CommandFailed(format!("Cannot read output: {}", e)))?;

        channel.wait_close().ok();

        Ok(output.trim().to_string())
    }

    /// Gets the current working directory on the remote server.
    ///
    /// # Errors
    /// Returns error if pwd command fails.
    pub fn get_cwd(&self) -> Result<String, SftpError> {
        self.exec_command("pwd")
    }

    /// Lists contents of a directory on the remote server.
    ///
    /// # Returns
    /// A vector of (filename, is_directory, size) tuples.
    ///
    /// # Errors
    /// Returns error if directory cannot be read.
    pub fn list_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, SftpError> {
        assert!(!path.is_empty(), "path must not be empty");

        let path = Path::new(path);
        let entries = self
            .sftp
            .readdir(path)
            .map_err(|e| SftpError::FileError(format!("Cannot read directory: {}", e)))?;

        let mut result = Vec::with_capacity(entries.len());

        for (entry_path, stat) in entries {
            let name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip hidden files starting with . (except .. for parent nav)
            // Actually, let's include all files for now and let the caller filter
            let is_dir = stat.is_dir();
            let size = stat.size.unwrap_or(0);

            result.push(RemoteDirEntry {
                name,
                is_directory: is_dir,
                size,
            });
        }

        // Sort: directories first, then alphabetically
        result.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(result)
    }

    /// Returns the SSH context used for this connection.
    #[must_use]
    pub fn context(&self) -> &SSHContext {
        &self.context
    }

    /// Checks if the connection is still alive.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.session.authenticated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_error_display() {
        let err = SftpError::ConnectionFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));

        let err = SftpError::FileTooLarge(20_000_000);
        assert!(err.to_string().contains("20000000"));
    }
}
