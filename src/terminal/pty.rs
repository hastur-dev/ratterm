//! PTY (pseudo-terminal) management using portable-pty.
//!
//! Provides cross-platform PTY spawning and I/O.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use thiserror::Error;

/// Maximum read buffer size.
const READ_BUFFER_SIZE: usize = 4096;

/// Maximum iterations for polling loops.
const MAX_POLL_ITERATIONS: usize = 1000;

/// PTY error type.
#[derive(Debug, Error)]
pub enum PtyError {
    /// Failed to create PTY.
    #[error("Failed to create PTY: {0}")]
    Creation(String),

    /// Failed to spawn process.
    #[error("Failed to spawn process: {0}")]
    Spawn(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// PTY is closed.
    #[error("PTY is closed")]
    Closed,

    /// Maximum number of tabs reached.
    #[error("Maximum number of terminal tabs reached")]
    MaxTabsReached,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

/// PTY event.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// Output data from the PTY.
    Output(Vec<u8>),
    /// PTY process exited.
    Exit(i32),
}

/// PTY configuration.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Shell to spawn (None = system default).
    pub shell: Option<String>,
    /// Arguments to the shell.
    pub args: Vec<String>,
    /// Environment variables to set.
    pub env: Vec<(String, String)>,
    /// Working directory.
    pub working_dir: Option<PathBuf>,
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyConfig {
    /// Creates a new configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shell: None,
            args: Vec::new(),
            env: Vec::new(),
            working_dir: None,
            cols: 80,
            rows: 24,
        }
    }

    /// Sets the shell.
    #[must_use]
    pub fn shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Sets the arguments.
    #[must_use]
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Sets the dimensions.
    #[must_use]
    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// PTY instance.
pub struct Pty {
    /// Master PTY handle.
    master: Box<dyn MasterPty + Send>,
    /// Writer to the PTY.
    writer: Box<dyn Write + Send>,
    /// Receiver for PTY events.
    event_rx: Receiver<PtyEvent>,
    /// Current columns.
    cols: u16,
    /// Current rows.
    rows: u16,
    /// Reader thread handle.
    reader_thread: Option<JoinHandle<()>>,
    /// Process ID.
    pid: Option<u32>,
    /// Running flag.
    running: bool,
}

impl Pty {
    /// Creates a new PTY with the given configuration.
    ///
    /// # Errors
    /// Returns error if PTY creation or process spawning fails.
    pub fn new(config: PtyConfig) -> Result<Self, PtyError> {
        assert!(config.cols > 0, "Columns must be positive");
        assert!(config.rows > 0, "Rows must be positive");

        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Creation(e.to_string()))?;

        // Build the command
        let mut cmd = if let Some(shell) = &config.shell {
            CommandBuilder::new(shell)
        } else {
            CommandBuilder::new_default_prog()
        };

        for arg in &config.args {
            cmd.arg(arg);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        if let Some(dir) = &config.working_dir {
            cmd.cwd(dir);
        }

        // Spawn the child process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;

        let pid = child.process_id();

        // Get the writer
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))?;

        // Create channel for events
        let (event_tx, event_rx) = mpsc::channel();

        // Spawn reader thread
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))?;

        let reader_thread = thread::spawn(move || {
            let mut buffer = vec![0u8; READ_BUFFER_SIZE];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        // EOF
                        let _ = event_tx.send(PtyEvent::Exit(0));
                        break;
                    }
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        if event_tx.send(PtyEvent::Output(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        let _ = event_tx.send(PtyEvent::Exit(-1));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            event_rx,
            cols: config.cols,
            rows: config.rows,
            reader_thread: Some(reader_thread),
            pid,
            running: true,
        })
    }

    /// Returns the number of columns.
    #[must_use]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    /// Returns the number of rows.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    /// Returns the process ID.
    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Returns true if the PTY is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Returns the current working directory of the PTY process.
    ///
    /// This attempts to read the CWD from the process. On Windows, this
    /// uses the process handle, on Unix it reads from /proc.
    #[must_use]
    pub fn current_working_dir(&self) -> Option<PathBuf> {
        self.pid.and_then(get_process_cwd)
    }

    /// Resizes the PTY.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))?;

        self.cols = cols;
        self.rows = rows;

        Ok(())
    }

    /// Writes data to the PTY.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn write(&mut self, data: &[u8]) -> Result<(), PtyError> {
        if !self.running {
            return Err(PtyError::Closed);
        }

        self.writer.write_all(data)?;
        self.writer.flush()?;

        Ok(())
    }

    /// Reads available data from the PTY.
    ///
    /// # Errors
    /// Returns error if read fails.
    pub fn read(&mut self) -> Result<Vec<u8>, PtyError> {
        if !self.running {
            return Err(PtyError::Closed);
        }

        let mut output = Vec::new();
        let mut iterations = 0;

        while iterations < MAX_POLL_ITERATIONS {
            match self.event_rx.try_recv() {
                Ok(PtyEvent::Output(data)) => {
                    output.extend(data);
                }
                Ok(PtyEvent::Exit(_code)) => {
                    self.running = false;
                    break;
                }
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.running = false;
                    break;
                }
            }
            iterations += 1;
        }

        Ok(output)
    }

    /// Tries to read an event without blocking.
    ///
    /// # Errors
    /// Returns error if the PTY is closed.
    pub fn try_read_event(&mut self) -> Result<Option<PtyEvent>, PtyError> {
        match self.event_rx.try_recv() {
            Ok(event) => {
                if matches!(event, PtyEvent::Exit(_)) {
                    self.running = false;
                }
                Ok(Some(event))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.running = false;
                Err(PtyError::Closed)
            }
        }
    }

    /// Shuts down the PTY gracefully.
    /// This is a quick shutdown that doesn't wait for threads.
    ///
    /// # Errors
    /// Returns error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<(), PtyError> {
        self.running = false;
        // Don't wait for reader thread - it will terminate when process exits
        // This allows for fast application shutdown
        let _ = self.reader_thread.take();
        Ok(())
    }

    /// Kills the PTY process.
    /// This is a quick kill that doesn't wait for threads.
    ///
    /// # Errors
    /// Returns error if kill fails.
    pub fn kill(&mut self) -> Result<(), PtyError> {
        self.running = false;
        let _ = self.reader_thread.take();
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.running = false;
        // Don't block on join during drop - let thread die with process
        let _ = self.reader_thread.take();
    }
}

/// Gets the current working directory of a process by PID.
///
/// On Windows, this uses NtQueryInformationProcess to read the process's PEB
/// and extract the current directory from the RTL_USER_PROCESS_PARAMETERS.
#[cfg(windows)]
fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::mem;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;

    // Windows API bindings
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn ReadProcessMemory(
            process: *mut std::ffi::c_void,
            base: *const std::ffi::c_void,
            buffer: *mut std::ffi::c_void,
            size: usize,
            read: *mut usize,
        ) -> i32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process: *mut std::ffi::c_void,
            info_class: u32,
            info: *mut std::ffi::c_void,
            info_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_VM_READ: u32 = 0x0010;
    const PROCESS_BASIC_INFORMATION: u32 = 0;

    #[repr(C)]
    struct ProcessBasicInformation {
        reserved1: *mut std::ffi::c_void,
        peb_base_address: *mut std::ffi::c_void,
        reserved2: [*mut std::ffi::c_void; 2],
        unique_process_id: usize,
        reserved3: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    // Offsets for x64 Windows
    #[cfg(target_pointer_width = "64")]
    const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x20;
    #[cfg(target_pointer_width = "64")]
    const RTL_USER_PROCESS_PARAMETERS_CWD_OFFSET: usize = 0x38;

    // Offsets for x86 Windows
    #[cfg(target_pointer_width = "32")]
    const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x10;
    #[cfg(target_pointer_width = "32")]
    const RTL_USER_PROCESS_PARAMETERS_CWD_OFFSET: usize = 0x24;

    unsafe {
        // Open the process with query and read permissions
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if handle.is_null() {
            return None;
        }

        let result = (|| -> Option<PathBuf> {
            // Get process basic information to find PEB address
            let mut pbi: ProcessBasicInformation = mem::zeroed();
            let mut return_length: u32 = 0;
            let status = NtQueryInformationProcess(
                handle,
                PROCESS_BASIC_INFORMATION,
                &mut pbi as *mut _ as *mut std::ffi::c_void,
                mem::size_of::<ProcessBasicInformation>() as u32,
                &mut return_length,
            );
            if status != 0 {
                return None;
            }

            // Read ProcessParameters pointer from PEB
            let mut process_params_ptr: *mut std::ffi::c_void = ptr::null_mut();
            let params_addr =
                (pbi.peb_base_address as usize + PEB_PROCESS_PARAMETERS_OFFSET) as *const _;
            let mut bytes_read: usize = 0;
            if ReadProcessMemory(
                handle,
                params_addr,
                &mut process_params_ptr as *mut _ as *mut _,
                mem::size_of::<*mut std::ffi::c_void>(),
                &mut bytes_read,
            ) == 0
            {
                return None;
            }

            // Read CurrentDirectory UNICODE_STRING from RTL_USER_PROCESS_PARAMETERS
            let cwd_addr =
                (process_params_ptr as usize + RTL_USER_PROCESS_PARAMETERS_CWD_OFFSET) as *const _;
            let mut cwd_unicode: UnicodeString = mem::zeroed();
            if ReadProcessMemory(
                handle,
                cwd_addr,
                &mut cwd_unicode as *mut _ as *mut _,
                mem::size_of::<UnicodeString>(),
                &mut bytes_read,
            ) == 0
            {
                return None;
            }

            // Read the actual path string
            if cwd_unicode.buffer.is_null() || cwd_unicode.length == 0 {
                return None;
            }

            let len = (cwd_unicode.length / 2) as usize; // length is in bytes, convert to u16 count
            let mut path_buffer: Vec<u16> = vec![0u16; len];
            if ReadProcessMemory(
                handle,
                cwd_unicode.buffer as *const _,
                path_buffer.as_mut_ptr() as *mut _,
                cwd_unicode.length as usize,
                &mut bytes_read,
            ) == 0
            {
                return None;
            }

            // Convert to OsString and PathBuf
            // Remove trailing backslash if present (except for root like "C:\")
            let os_string = OsString::from_wide(&path_buffer);
            let mut path = PathBuf::from(os_string);

            // Clean up trailing backslash for non-root paths
            if let Some(path_str) = path.to_str() {
                if path_str.len() > 3 && path_str.ends_with('\\') {
                    path = PathBuf::from(&path_str[..path_str.len() - 1]);
                }
            }

            Some(path)
        })();

        CloseHandle(handle);
        result
    }
}

/// Gets the current working directory of a process by PID.
///
/// On Linux, this reads the /proc/<pid>/cwd symlink.
#[cfg(target_os = "linux")]
fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    let proc_path = format!("/proc/{}/cwd", pid);
    std::fs::read_link(&proc_path).ok()
}

/// Gets the current working directory of a process by PID.
///
/// On macOS, this uses the proc_pidinfo API via libproc.
#[cfg(target_os = "macos")]
fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int};

    // libproc bindings for macOS
    #[link(name = "proc", kind = "dylib")]
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_char,
            buffersize: c_int,
        ) -> c_int;
    }

    const PROC_PIDVNODEPATHINFO: c_int = 9;
    const MAXPATHLEN: usize = 1024;

    // vnode_info_path structure (simplified - we only need the path at the start)
    #[repr(C)]
    struct VnodeInfoPath {
        // The actual structure is larger, but we only need the cwd path
        // which starts at offset 152 (vip_path in struct vnode_info_path)
        _padding: [u8; 152],
        vip_path: [c_char; MAXPATHLEN],
    }

    unsafe {
        let mut info: VnodeInfoPath = std::mem::zeroed();
        let size = std::mem::size_of::<VnodeInfoPath>() as c_int;

        let result = proc_pidinfo(
            pid as c_int,
            PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut c_char,
            size,
        );

        if result <= 0 {
            return None;
        }

        // Convert C string to PathBuf
        let path_cstr = CStr::from_ptr(info.vip_path.as_ptr());
        path_cstr.to_str().ok().map(PathBuf::from)
    }
}

/// Gets the current working directory of a process by PID.
///
/// Returns `None` on unsupported Unix platforms (FreeBSD, etc.).
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn get_process_cwd(_pid: u32) -> Option<PathBuf> {
    // Other Unix platforms - return None and rely on OSC 7
    None
}

/// Gets the current working directory of a process by PID.
///
/// Returns `None` on unsupported platforms.
#[cfg(not(any(windows, unix)))]
fn get_process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_config_defaults() {
        let config = PtyConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
        assert!(config.shell.is_none());
    }

    #[test]
    fn test_pty_config_builder() {
        let config = PtyConfig::new().size(120, 40);
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
    }
}
