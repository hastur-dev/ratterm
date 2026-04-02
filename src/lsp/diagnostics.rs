//! Diagnostics from LSP (errors, warnings, info).

use super::definition::uri_to_path;
use super::hover::TextRange;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    /// Parses severity from LSP integer (1=Error, 2=Warning, 3=Info, 4=Hint).
    pub fn from_lsp(value: u64) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Information,
            _ => Self::Hint,
        }
    }

    /// Returns the gutter character for this severity.
    pub fn gutter_char(&self) -> char {
        match self {
            Self::Error => 'E',
            Self::Warning => 'W',
            Self::Information => 'I',
            Self::Hint => 'H',
        }
    }

    /// Returns a short label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "info",
            Self::Hint => "hint",
        }
    }
}

/// A single diagnostic info (used for code action context).
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,
}

/// Storage for diagnostics across files.
#[derive(Debug, Clone)]
pub struct DiagnosticStore {
    /// Diagnostics per file path.
    store: Arc<RwLock<HashMap<PathBuf, Vec<DiagnosticInfo>>>>,
}

impl DiagnosticStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Updates diagnostics for a file from a `publishDiagnostics` notification.
    pub fn update_from_notification(&self, params: &JsonValue) {
        let Some(uri) = params.get("uri").and_then(|u| u.as_str()) else {
            return;
        };
        let Some(path) = uri_to_path(uri) else {
            return;
        };
        let diagnostics = params
            .get("diagnostics")
            .and_then(|d| d.as_array())
            .map(|arr| parse_diagnostics(arr))
            .unwrap_or_default();

        if let Ok(mut store) = self.store.write() {
            if diagnostics.is_empty() {
                store.remove(&path);
            } else {
                store.insert(path, diagnostics);
            }
        }
    }

    /// Returns diagnostics for a file.
    pub fn get(&self, path: &std::path::Path) -> Vec<DiagnosticInfo> {
        self.store
            .read()
            .ok()
            .and_then(|s| s.get(path).cloned())
            .unwrap_or_default()
    }

    /// Returns all diagnostics across all files.
    pub fn all(&self) -> HashMap<PathBuf, Vec<DiagnosticInfo>> {
        self.store
            .read()
            .ok()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Returns total count of diagnostics.
    pub fn total_count(&self) -> usize {
        self.store
            .read()
            .ok()
            .map(|s| s.values().map(|v| v.len()).sum())
            .unwrap_or(0)
    }

    /// Returns diagnostics for a specific line in a file (owned).
    pub fn for_line_owned(&self, path: &std::path::Path, line: u32) -> Vec<DiagnosticInfo> {
        self.get(path)
            .into_iter()
            .filter(|d| d.range.start_line <= line && d.range.end_line >= line)
            .collect()
    }

    /// Clears all diagnostics.
    pub fn clear(&self) {
        if let Ok(mut store) = self.store.write() {
            store.clear();
        }
    }
}

impl Default for DiagnosticStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses an array of LSP diagnostic objects.
pub fn parse_diagnostics(arr: &[JsonValue]) -> Vec<DiagnosticInfo> {
    arr.iter().filter_map(parse_single_diagnostic).collect()
}

fn parse_single_diagnostic(val: &JsonValue) -> Option<DiagnosticInfo> {
    let range_val = val.get("range")?;
    let range = super::hover::parse_range(range_val)?;
    let message = val.get("message")?.as_str()?.to_string();
    let severity = val
        .get("severity")
        .and_then(|s| s.as_u64())
        .map(DiagnosticSeverity::from_lsp)
        .unwrap_or(DiagnosticSeverity::Error);
    let code = val.get("code").and_then(|c| {
        c.as_str()
            .map(|s| s.to_string())
            .or_else(|| c.as_u64().map(|n| n.to_string()))
    });
    let source = val
        .get("source")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    Some(DiagnosticInfo {
        range,
        severity,
        message,
        code,
        source,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_diagnostics() {
        let arr = vec![
            json!({
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}},
                "severity": 1,
                "message": "expected `;`",
                "code": "E0308",
                "source": "rustc"
            }),
            json!({
                "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 5}},
                "severity": 2,
                "message": "unused variable",
                "source": "rustc"
            }),
        ];
        let diagnostics = parse_diagnostics(&arr);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].message, "expected `;`");
    }

    #[test]
    fn test_diagnostic_store() {
        let store = DiagnosticStore::new();
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}},
                    "severity": 1,
                    "message": "error here"
                }
            ]
        });
        store.update_from_notification(&notification);
        assert_eq!(store.total_count(), 1);
    }

    #[test]
    fn test_diagnostic_severity() {
        assert_eq!(DiagnosticSeverity::from_lsp(1), DiagnosticSeverity::Error);
        assert_eq!(DiagnosticSeverity::from_lsp(2), DiagnosticSeverity::Warning);
        assert_eq!(DiagnosticSeverity::Error.gutter_char(), 'E');
        assert_eq!(DiagnosticSeverity::Warning.label(), "warning");
    }

    #[test]
    fn test_diagnostic_store_clear() {
        let store = DiagnosticStore::new();
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
                "severity": 1,
                "message": "err"
            }]
        });
        store.update_from_notification(&notification);
        assert_eq!(store.total_count(), 1);
        store.clear();
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn test_diagnostic_store_empty_clears_file() {
        let store = DiagnosticStore::new();
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
                "severity": 1,
                "message": "err"
            }]
        });
        store.update_from_notification(&notification);
        assert_eq!(store.total_count(), 1);

        // Update with empty diagnostics should remove the file entry
        let clear_notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": []
        });
        store.update_from_notification(&clear_notification);
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn test_diagnostic_for_line_owned() {
        let store = DiagnosticStore::new();
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 10}},
                    "severity": 1,
                    "message": "error on line 5"
                },
                {
                    "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 5}},
                    "severity": 2,
                    "message": "warning on line 10"
                }
            ]
        });
        store.update_from_notification(&notification);

        #[cfg(not(windows))]
        {
            let line_5 = store.for_line_owned(std::path::Path::new("/src/main.rs"), 5);
            assert_eq!(line_5.len(), 1);
            assert_eq!(line_5[0].message, "error on line 5");
        }
    }

    #[test]
    fn test_diagnostic_code_as_number() {
        let arr = vec![json!({
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
            "severity": 1,
            "message": "err",
            "code": 42
        })];
        let diagnostics = parse_diagnostics(&arr);
        assert_eq!(diagnostics[0].code.as_deref(), Some("42"));
    }
}
