//! Unix domain socket transport implementation.

use crate::api::transport::{default_socket_path, BufferedConnection, Connection};
use crate::api::ApiError;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use tracing::{debug, error, info};

/// Unix domain socket server.
pub struct UnixServer {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl UnixServer {
    /// Creates a new Unix socket server.
    pub fn new(socket_path: Option<PathBuf>) -> Result<Self, ApiError> {
        let path = socket_path.unwrap_or_else(default_socket_path);

        // Remove stale socket file if it exists
        if path.exists() {
            debug!("Removing stale socket file: {:?}", path);
            let _ = std::fs::remove_file(&path);
        }

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Bind the listener
        let listener = UnixListener::bind(&path)?;

        // Set socket permissions (owner-only for security)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        info!("API server listening on: {:?}", path);

        Ok(Self {
            listener,
            socket_path: path,
        })
    }

    /// Returns the socket path.
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Accepts a new connection (blocking).
    pub fn accept(&self) -> Result<UnixConnection, ApiError> {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                debug!("Client connected: {:?}", addr);
                Ok(UnixConnection::new(stream))
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                Err(ApiError::Transport(e))
            }
        }
    }

    /// Sets the listener to non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), ApiError> {
        self.listener.set_nonblocking(nonblocking)?;
        Ok(())
    }
}

impl Drop for UnixServer {
    fn drop(&mut self) {
        // Clean up socket file
        if self.socket_path.exists() {
            debug!("Removing socket file: {:?}", self.socket_path);
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// Unix domain socket connection.
pub struct UnixConnection {
    inner: BufferedConnection<BufReader<UnixStream>, BufWriter<UnixStream>>,
}

impl UnixConnection {
    /// Creates a new Unix connection from a stream.
    pub fn new(stream: UnixStream) -> Self {
        // Clone the stream for separate read/write handles
        let read_stream = stream.try_clone().expect("Failed to clone stream");
        let write_stream = stream;

        Self {
            inner: BufferedConnection::new(
                BufReader::new(read_stream),
                BufWriter::new(write_stream),
            ),
        }
    }
}

impl Connection for UnixConnection {
    fn read_message(&mut self) -> Result<Option<String>, ApiError> {
        self.inner.read_message()
    }

    fn write_message(&mut self, msg: &str) -> Result<(), ApiError> {
        self.inner.write_message(msg)
    }

    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

/// Client connection to a Unix socket (for testing).
pub struct UnixClient {
    stream: UnixStream,
}

impl UnixClient {
    /// Connects to a Unix socket server.
    pub fn connect(socket_path: &std::path::Path) -> Result<Self, ApiError> {
        let stream = UnixStream::connect(socket_path)?;
        Ok(Self { stream })
    }

    /// Creates a buffered connection from this client.
    pub fn into_connection(self) -> UnixConnection {
        UnixConnection::new(self.stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_server_client_communication() {
        let socket_path = std::env::temp_dir().join("ratterm-test.sock");

        // Clean up any existing socket
        let _ = std::fs::remove_file(&socket_path);

        // Create server
        let server = UnixServer::new(Some(socket_path.clone())).unwrap();

        // Spawn client thread
        let client_path = socket_path.clone();
        let client_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));

            let client = UnixClient::connect(&client_path).unwrap();
            let mut conn = client.into_connection();

            conn.write_message(r#"{"id":"1","method":"test"}"#).unwrap();

            let response = conn.read_message().unwrap();
            assert!(response.is_some());
        });

        // Accept connection
        let mut conn = server.accept().unwrap();

        // Read message
        let msg = conn.read_message().unwrap();
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("test"));

        // Send response
        conn.write_message(r#"{"id":"1","result":{}}"#).unwrap();

        client_thread.join().unwrap();
    }
}
