//! Windows Named Pipe transport implementation.

use crate::api::ApiError;
use crate::api::transport::{BufferedConnection, Connection, DEFAULT_PIPE_NAME};
use std::ffi::OsStr;
use std::io::{BufReader, BufWriter};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use tracing::{debug, error, info};

// Windows API constants
const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const INVALID_HANDLE_VALUE: isize = -1;
const ERROR_PIPE_CONNECTED: u32 = 535;
const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const OPEN_EXISTING: u32 = 3;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(
        lpName: *const u16,
        dwOpenMode: u32,
        dwPipeMode: u32,
        nMaxInstances: u32,
        nOutBufferSize: u32,
        nInBufferSize: u32,
        nDefaultTimeOut: u32,
        lpSecurityAttributes: *mut std::ffi::c_void,
    ) -> isize;

    fn ConnectNamedPipe(hNamedPipe: isize, lpOverlapped: *mut std::ffi::c_void) -> i32;

    fn DisconnectNamedPipe(hNamedPipe: isize) -> i32;

    fn CloseHandle(hObject: isize) -> i32;

    fn GetLastError() -> u32;

    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut std::ffi::c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: isize,
    ) -> isize;
}

/// Converts a Rust string to a null-terminated wide string.
fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Windows named pipe handle wrapper.
struct PipeHandle(isize);

impl PipeHandle {
    fn is_valid(&self) -> bool {
        self.0 != INVALID_HANDLE_VALUE && self.0 != 0
    }
}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Windows named pipe server.
pub struct WindowsServer {
    pipe_name: String,
}

impl WindowsServer {
    /// Creates a new Windows pipe server.
    pub fn new(pipe_name: Option<&str>) -> Self {
        Self {
            pipe_name: pipe_name.unwrap_or(DEFAULT_PIPE_NAME).to_string(),
        }
    }

    /// Returns the pipe name.
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    /// Accepts a new connection (blocking).
    pub fn accept(&self) -> Result<WindowsConnection, ApiError> {
        let wide_name = to_wide_string(&self.pipe_name);

        // Create the named pipe
        let handle = unsafe {
            CreateNamedPipeW(
                wide_name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                4096, // Output buffer size
                4096, // Input buffer size
                0,    // Default timeout
                ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            error!("Failed to create named pipe: error {}", err);
            return Err(ApiError::Transport(std::io::Error::from_raw_os_error(
                err as i32,
            )));
        }

        debug!("Created named pipe: {}", self.pipe_name);

        // Wait for client connection
        let result = unsafe { ConnectNamedPipe(handle, ptr::null_mut()) };

        if result == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_PIPE_CONNECTED {
                unsafe { CloseHandle(handle) };
                error!("Failed to connect named pipe: error {}", err);
                return Err(ApiError::Transport(std::io::Error::from_raw_os_error(
                    err as i32,
                )));
            }
        }

        info!("Client connected to named pipe");

        Ok(WindowsConnection {
            handle: PipeHandle(handle),
            reader: None,
            writer: None,
            open: true,
        })
    }
}

/// Windows named pipe connection.
pub struct WindowsConnection {
    handle: PipeHandle,
    reader: Option<BufReader<PipeReader>>,
    writer: Option<BufWriter<PipeWriter>>,
    /// Tracks if the connection is still open (client hasn't disconnected).
    open: bool,
}

impl WindowsConnection {
    /// Initializes readers/writers (must be called before use).
    pub fn init(&mut self) -> Result<(), ApiError> {
        if !self.handle.is_valid() {
            return Err(ApiError::Transport(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Invalid pipe handle",
            )));
        }

        // Create reader and writer wrappers
        let reader = PipeReader(self.handle.0);
        let writer = PipeWriter(self.handle.0);

        self.reader = Some(BufReader::new(reader));
        self.writer = Some(BufWriter::new(writer));

        Ok(())
    }
}

impl Connection for WindowsConnection {
    fn read_message(&mut self) -> Result<Option<String>, ApiError> {
        if !self.open {
            return Ok(None);
        }

        if self.reader.is_none() {
            self.init()?;
        }

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| ApiError::Internal("Reader not initialized".to_string()))?;
        let mut line = String::new();

        use std::io::BufRead;
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF - client disconnected
                self.open = false;
                Ok(None)
            }
            Ok(_) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => {
                self.open = false;
                Err(ApiError::Transport(e))
            }
        }
    }

    fn write_message(&mut self, msg: &str) -> Result<(), ApiError> {
        if self.writer.is_none() {
            self.init()?;
        }

        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| ApiError::Internal("Writer not initialized".to_string()))?;
        use std::io::Write;
        writeln!(writer, "{}", msg)?;
        writer.flush()?;
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open && self.handle.is_valid()
    }
}

impl Drop for WindowsConnection {
    fn drop(&mut self) {
        if self.handle.is_valid() {
            unsafe {
                DisconnectNamedPipe(self.handle.0);
            }
        }
    }
}

/// Pipe reader wrapper for std::io::Read.
pub struct PipeReader(isize);

impl std::io::Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut bytes_read: u32 = 0;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn ReadFile(
                hFile: isize,
                lpBuffer: *mut u8,
                nNumberOfBytesToRead: u32,
                lpNumberOfBytesRead: *mut u32,
                lpOverlapped: *mut std::ffi::c_void,
            ) -> i32;
        }

        let result = unsafe {
            ReadFile(
                self.0,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut bytes_read,
                ptr::null_mut(),
            )
        };

        if result == 0 {
            let err = unsafe { GetLastError() };
            if err == 109 {
                // ERROR_BROKEN_PIPE
                return Ok(0);
            }
            return Err(std::io::Error::from_raw_os_error(err as i32));
        }

        Ok(bytes_read as usize)
    }
}

/// Pipe writer wrapper for std::io::Write.
pub struct PipeWriter(isize);

impl std::io::Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut bytes_written: u32 = 0;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn WriteFile(
                hFile: isize,
                lpBuffer: *const u8,
                nNumberOfBytesToWrite: u32,
                lpNumberOfBytesWritten: *mut u32,
                lpOverlapped: *mut std::ffi::c_void,
            ) -> i32;
        }

        let result = unsafe {
            WriteFile(
                self.0,
                buf.as_ptr(),
                buf.len() as u32,
                &mut bytes_written,
                ptr::null_mut(),
            )
        };

        if result == 0 {
            let err = unsafe { GetLastError() };
            return Err(std::io::Error::from_raw_os_error(err as i32));
        }

        Ok(bytes_written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn FlushFileBuffers(hFile: isize) -> i32;
        }

        let result = unsafe { FlushFileBuffers(self.0) };
        if result == 0 {
            let err = unsafe { GetLastError() };
            return Err(std::io::Error::from_raw_os_error(err as i32));
        }
        Ok(())
    }
}

/// Client connection to a named pipe (for testing).
pub struct WindowsClient {
    handle: PipeHandle,
}

impl WindowsClient {
    /// Connects to a named pipe server.
    pub fn connect(pipe_name: &str) -> Result<Self, ApiError> {
        let wide_name = to_wide_string(pipe_name);

        let handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(ApiError::Transport(std::io::Error::from_raw_os_error(
                err as i32,
            )));
        }

        Ok(Self {
            handle: PipeHandle(handle),
        })
    }

    /// Creates a buffered connection from this client.
    pub fn into_connection(
        self,
    ) -> BufferedConnection<BufReader<PipeReader>, BufWriter<PipeWriter>> {
        let reader = BufReader::new(PipeReader(self.handle.0));
        let writer = BufWriter::new(PipeWriter(self.handle.0));

        // Prevent drop from closing handle
        std::mem::forget(self.handle);

        BufferedConnection::new(reader, writer)
    }
}
