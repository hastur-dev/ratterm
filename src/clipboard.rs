//! Clipboard module for copy/paste operations.
//!
//! Provides cross-platform clipboard support using arboard.

use std::sync::{Arc, Mutex};

/// Clipboard manager.
#[derive(Debug, Clone)]
pub struct Clipboard {
    /// Internal clipboard content (fallback if system clipboard unavailable).
    internal: Arc<Mutex<String>>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    /// Creates a new clipboard manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            internal: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Copies text to the clipboard.
    ///
    /// # Errors
    /// Returns error if clipboard access fails.
    pub fn copy(&self, text: &str) -> Result<(), ClipboardError> {
        // Always update internal clipboard first (used as cache and for has_content check)
        if let Ok(mut internal) = self.internal.lock() {
            *internal = text.to_string();
        } else {
            return Err(ClipboardError::LockFailed);
        }

        // Also try system clipboard if available
        #[cfg(feature = "system-clipboard")]
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                // Ignore system clipboard errors - we still have internal copy
                let _ = clipboard.set_text(text);
            }
        }

        Ok(())
    }

    /// Pastes text from the clipboard.
    ///
    /// # Errors
    /// Returns error if clipboard access fails.
    pub fn paste(&self) -> Result<String, ClipboardError> {
        // Try system clipboard first
        #[cfg(feature = "system-clipboard")]
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    return Ok(text);
                }
            }
        }

        // Fall back to internal clipboard
        if let Ok(internal) = self.internal.lock() {
            Ok(internal.clone())
        } else {
            Err(ClipboardError::LockFailed)
        }
    }

    /// Checks if the clipboard has content.
    #[must_use]
    pub fn has_content(&self) -> bool {
        if let Ok(internal) = self.internal.lock() {
            !internal.is_empty()
        } else {
            false
        }
    }
}

/// Clipboard errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardError {
    /// Failed to access system clipboard.
    SystemUnavailable,
    /// Failed to acquire lock.
    LockFailed,
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemUnavailable => write!(f, "System clipboard unavailable"),
            Self::LockFailed => write!(f, "Failed to acquire clipboard lock"),
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_clipboard_internal() {
        let clipboard = Clipboard::new();
        assert!(!clipboard.has_content());

        clipboard.copy("Hello, World!").unwrap();
        assert!(clipboard.has_content());

        let text = clipboard.paste().unwrap();
        assert_eq!(text, "Hello, World!");
    }
}
