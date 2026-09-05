//! Authentication for the control API.
//!
//! The IPC endpoint used to be unauthenticated: any local process could open
//! `\\.\pipe\ratterm-api` (or `/tmp/ratterm-api.sock`), read the editor buffer,
//! and inject keystrokes into the PTY. On a shared machine that is a remote
//! shell handed to whoever asks first.
//!
//! Each run now mints a token, writes it to a file only the user can read, and
//! requires it on the first message of every connection. The handshake is one
//! extra field, so a client that already speaks the protocol only has to read
//! the file:
//!
//! ```json
//! {"id":"1","method":"session.authenticate","params":{"token":"<hex>"}}
//! ```
//!
//! Until that succeeds, every other method is refused.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;
use zeroize::Zeroizing;

/// Token length in bytes before hex encoding.
const TOKEN_BYTES: usize = 32;

/// Method a client calls to present its token.
pub const AUTHENTICATE_METHOD: &str = "session.authenticate";

/// Methods a client may call before authenticating.
///
/// `system.ping` stays open so a client can tell "ratterm is not running" from
/// "ratterm is running and I have the wrong token".
pub const UNAUTHENTICATED_METHODS: &[&str] = &[AUTHENTICATE_METHOD, "system.ping"];

/// Errors raised while handling the token.
#[derive(Debug, Error)]
pub enum AuthError {
    /// The token file could not be read or written.
    #[error("token file error: {0}")]
    Io(#[from] std::io::Error),

    /// System randomness was unavailable.
    #[error("system randomness unavailable: {0}")]
    Random(String),

    /// The token file did not contain a usable token.
    #[error("token file is malformed")]
    Malformed,
}

/// A per-session API token.
///
/// Compared in constant time, and never printed by `Debug`.
#[derive(Clone)]
pub struct SessionToken {
    hex: Zeroizing<String>,
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

impl SessionToken {
    /// Mints a fresh token from operating-system randomness.
    ///
    /// # Errors
    /// Returns an error if the system random source is unavailable.
    pub fn generate() -> Result<Self, AuthError> {
        let mut bytes = Zeroizing::new([0u8; TOKEN_BYTES]);
        getrandom::fill(bytes.as_mut()).map_err(|e| AuthError::Random(e.to_string()))?;

        let mut hex = String::with_capacity(TOKEN_BYTES * 2);
        for byte in bytes.iter() {
            use std::fmt::Write as _;
            // Writing to a String cannot fail.
            let _ = write!(hex, "{byte:02x}");
        }

        Ok(Self {
            hex: Zeroizing::new(hex),
        })
    }

    /// Builds a token from an existing hex string.
    ///
    /// # Errors
    /// Returns [`AuthError::Malformed`] if the string is not the right length
    /// or contains non-hex characters.
    pub fn from_hex(hex: &str) -> Result<Self, AuthError> {
        let trimmed = hex.trim();
        if trimmed.len() != TOKEN_BYTES * 2 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AuthError::Malformed);
        }
        Ok(Self {
            hex: Zeroizing::new(trimmed.to_ascii_lowercase()),
        })
    }

    /// Returns the token as a hex string.
    ///
    /// Handle with care: anything that can read this can drive the editor and
    /// the terminal.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.hex
    }

    /// Compares against a candidate without leaking the answer through timing.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        constant_time_eq(self.hex.as_bytes(), candidate.trim().as_bytes())
    }

    /// Default location of the token file.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("api.token")
    }

    /// Writes the token to `path`, restricted to the owner where the platform
    /// allows it.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn write_to(&self, path: &Path) -> Result<(), AuthError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        // Create with restrictive permissions from the start on Unix, so there
        // is no window where the token is world readable.
        #[cfg(unix)]
        {
            use std::io::Write as _;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            file.write_all(self.hex.as_bytes())?;
            file.flush()?;
        }
        #[cfg(not(unix))]
        {
            fs::write(path, self.hex.as_bytes())?;
        }

        crate::secrets::vault::restrict_permissions_public(path)?;
        Ok(())
    }

    /// Reads a token written by [`SessionToken::write_to`].
    ///
    /// # Errors
    /// Returns an error if the file is missing or malformed.
    pub fn read_from(path: &Path) -> Result<Self, AuthError> {
        let text = fs::read_to_string(path)?;
        Self::from_hex(&text)
    }

    /// Removes the token file, ignoring a missing file.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be removed.
    pub fn remove_file(path: &Path) -> Result<(), AuthError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AuthError::Io(e)),
        }
    }
}

/// Byte comparison whose running time does not depend on where the first
/// difference is.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Per-connection authentication state.
#[derive(Debug)]
pub struct ConnectionAuth {
    /// The token this session expects, or `None` when authentication is off.
    expected: Option<SessionToken>,
    /// Whether this connection has presented the token.
    authenticated: bool,
}

impl ConnectionAuth {
    /// Creates state requiring `token`.
    #[must_use]
    pub fn requiring(token: SessionToken) -> Self {
        Self {
            expected: Some(token),
            authenticated: false,
        }
    }

    /// Creates state that accepts everything.
    ///
    /// Only for callers that have already established trust by other means,
    /// such as an in-process test harness.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            expected: None,
            authenticated: true,
        }
    }

    /// Returns true if authentication is enforced.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.expected.is_some()
    }

    /// Returns true once the client has presented the token.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Records a successful handshake if `candidate` matches.
    pub fn authenticate(&mut self, candidate: &str) -> bool {
        match &self.expected {
            None => true,
            Some(token) => {
                if token.matches(candidate) {
                    self.authenticated = true;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Returns true if `method` may run on this connection right now.
    #[must_use]
    pub fn permits(&self, method: &str) -> bool {
        self.authenticated || UNAUTHENTICATED_METHODS.contains(&method)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_token_is_hex_of_the_expected_length() {
        let token = SessionToken::generate().expect("generate");
        assert_eq!(token.as_hex().len(), TOKEN_BYTES * 2);
        assert!(token.as_hex().bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn two_generated_tokens_differ() {
        let a = SessionToken::generate().expect("a");
        let b = SessionToken::generate().expect("b");
        assert_ne!(a.as_hex(), b.as_hex());
    }

    #[test]
    fn debug_output_does_not_leak_the_token() {
        let token = SessionToken::generate().expect("generate");
        let rendered = format!("{token:?}");
        assert!(!rendered.contains(token.as_hex()));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn a_token_matches_itself_and_nothing_else() {
        let token = SessionToken::generate().expect("generate");
        assert!(token.matches(token.as_hex()));
        assert!(token.matches(&format!("  {}\n", token.as_hex())));
        assert!(!token.matches("00"));
        assert!(!token.matches(""));
        assert!(!token.matches(&"f".repeat(TOKEN_BYTES * 2)));
    }

    #[test]
    fn hex_parsing_rejects_the_wrong_shape() {
        assert!(SessionToken::from_hex("abc").is_err());
        assert!(SessionToken::from_hex(&"z".repeat(TOKEN_BYTES * 2)).is_err());
        assert!(SessionToken::from_hex(&"a".repeat(TOKEN_BYTES * 2)).is_ok());
    }

    #[test]
    fn hex_parsing_is_case_insensitive() {
        let upper = "A".repeat(TOKEN_BYTES * 2);
        let token = SessionToken::from_hex(&upper).expect("parse");
        assert!(token.matches(&"a".repeat(TOKEN_BYTES * 2)));
    }

    #[test]
    fn a_token_round_trips_through_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api.token");

        let token = SessionToken::generate().expect("generate");
        token.write_to(&path).expect("write");

        let read_back = SessionToken::read_from(&path).expect("read");
        assert!(token.matches(read_back.as_hex()));
    }

    #[cfg(unix)]
    #[test]
    fn the_token_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api.token");
        SessionToken::generate()
            .expect("generate")
            .write_to(&path)
            .expect("write");

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "token file must be 0600");
    }

    #[test]
    fn reading_a_missing_token_file_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(SessionToken::read_from(&dir.path().join("absent")).is_err());
    }

    #[test]
    fn reading_a_garbage_token_file_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api.token");
        std::fs::write(&path, "not a token").expect("write");
        assert!(matches!(
            SessionToken::read_from(&path),
            Err(AuthError::Malformed)
        ));
    }

    #[test]
    fn removing_an_absent_token_file_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        SessionToken::remove_file(&dir.path().join("absent")).expect("remove");
    }

    #[test]
    fn removing_the_token_file_deletes_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api.token");
        SessionToken::generate()
            .expect("generate")
            .write_to(&path)
            .expect("write");
        assert!(path.exists());
        SessionToken::remove_file(&path).expect("remove");
        assert!(!path.exists());
    }

    #[test]
    fn an_unauthenticated_connection_only_allows_the_handshake_and_ping() {
        let token = SessionToken::generate().expect("generate");
        let auth = ConnectionAuth::requiring(token);

        assert!(auth.is_required());
        assert!(!auth.is_authenticated());
        assert!(auth.permits(AUTHENTICATE_METHOD));
        assert!(auth.permits("system.ping"));
        assert!(!auth.permits("terminal.send_keys"));
        assert!(!auth.permits("editor.read_content"));
    }

    #[test]
    fn presenting_the_token_unlocks_every_method() {
        let token = SessionToken::generate().expect("generate");
        let hex = token.as_hex().to_string();
        let mut auth = ConnectionAuth::requiring(token);

        assert!(auth.authenticate(&hex));
        assert!(auth.is_authenticated());
        assert!(auth.permits("terminal.send_keys"));
    }

    #[test]
    fn a_wrong_token_leaves_the_connection_locked() {
        let token = SessionToken::generate().expect("generate");
        let mut auth = ConnectionAuth::requiring(token);

        assert!(!auth.authenticate("deadbeef"));
        assert!(!auth.is_authenticated());
        assert!(!auth.permits("terminal.send_keys"));
    }

    #[test]
    fn repeated_wrong_attempts_do_not_unlock() {
        let token = SessionToken::generate().expect("generate");
        let mut auth = ConnectionAuth::requiring(token);
        for _ in 0..16 {
            assert!(!auth.authenticate("00"));
        }
        assert!(!auth.is_authenticated());
    }

    #[test]
    fn disabled_authentication_permits_everything() {
        let mut auth = ConnectionAuth::disabled();
        assert!(!auth.is_required());
        assert!(auth.is_authenticated());
        assert!(auth.permits("terminal.send_keys"));
        assert!(auth.authenticate("anything"));
    }

    #[test]
    fn constant_time_eq_matches_ordinary_equality() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
