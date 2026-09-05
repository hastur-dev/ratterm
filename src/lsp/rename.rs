//! Symbol rename functionality.

use super::definition::uri_to_path;
use super::hover::TextRange;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Range where rename is valid.
#[derive(Debug, Clone)]
pub struct RenameRange {
    pub range: TextRange,
    pub placeholder: String,
}

/// Result of a workspace edit (rename operation).
#[derive(Debug, Clone)]
pub struct WorkspaceEditResult {
    /// File path -> list of text edits.
    pub changes: BTreeMap<PathBuf, Vec<TextEditInfo>>,
}

/// A single text edit within a file.
#[derive(Debug, Clone)]
pub struct TextEditInfo {
    pub range: TextRange,
    pub new_text: String,
}

/// Parses a prepare rename response.
pub fn parse_prepare_rename(result: JsonValue) -> Option<RenameRange> {
    if result.is_null() {
        return None;
    }

    // Response can be: Range | { range: Range, placeholder: string } | { defaultBehavior: boolean }
    if let Some(range_val) = result.get("range") {
        let range = super::hover::parse_range(range_val)?;
        let placeholder = result
            .get("placeholder")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        Some(RenameRange { range, placeholder })
    } else if result.get("start").is_some() {
        // Direct range
        let range = super::hover::parse_range(&result)?;
        Some(RenameRange {
            range,
            placeholder: String::new(),
        })
    } else {
        None
    }
}

/// Parses a workspace edit response.
pub fn parse_workspace_edit(result: JsonValue) -> Option<WorkspaceEditResult> {
    if result.is_null() {
        return None;
    }

    let mut changes = BTreeMap::new();

    // Handle "changes" field: { [uri]: TextEdit[] }
    if let Some(changes_obj) = result.get("changes").and_then(|c| c.as_object()) {
        for (uri, edits_val) in changes_obj {
            if let Some(path) = uri_to_path(uri) {
                let edits = parse_text_edits(edits_val);
                if !edits.is_empty() {
                    changes.insert(path, edits);
                }
            }
        }
    }

    // Handle "documentChanges" field: TextDocumentEdit[]
    if let Some(doc_changes) = result.get("documentChanges").and_then(|d| d.as_array()) {
        for doc_change in doc_changes {
            if let Some(text_doc) = doc_change.get("textDocument")
                && let Some(uri) = text_doc.get("uri").and_then(|u| u.as_str())
                && let Some(path) = uri_to_path(uri)
                && let Some(edits_val) = doc_change.get("edits")
            {
                let edits = parse_text_edits(edits_val);
                if !edits.is_empty() {
                    changes.entry(path).or_default().extend(edits);
                }
            }
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(WorkspaceEditResult { changes })
    }
}

fn parse_text_edits(val: &JsonValue) -> Vec<TextEditInfo> {
    let Some(arr) = val.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|edit| {
            let range = super::hover::parse_range(edit.get("range")?)?;
            let new_text = edit.get("newText")?.as_str()?.to_string();
            Some(TextEditInfo { range, new_text })
        })
        .collect()
}

/// Counts total edits across all files.
pub fn total_edit_count(edit: &WorkspaceEditResult) -> usize {
    edit.changes.values().map(|edits| edits.len()).sum()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_prepare_rename_with_placeholder() {
        let resp = json!({
            "range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 10}},
            "placeholder": "old_name"
        });
        let result = parse_prepare_rename(resp).unwrap();
        assert_eq!(result.placeholder, "old_name");
        assert_eq!(result.range.start_line, 5);
    }

    #[test]
    fn test_parse_prepare_rename_direct_range() {
        let resp = json!({
            "start": {"line": 3, "character": 0},
            "end": {"line": 3, "character": 6}
        });
        let result = parse_prepare_rename(resp).unwrap();
        assert_eq!(result.range.start_line, 3);
        assert!(result.placeholder.is_empty());
    }

    #[test]
    fn test_parse_workspace_edit() {
        let resp = json!({
            "changes": {
                "file:///src/main.rs": [
                    {"range": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 10}}, "newText": "new_name"},
                    {"range": {"start": {"line": 5, "character": 8}, "end": {"line": 5, "character": 14}}, "newText": "new_name"}
                ],
                "file:///src/lib.rs": [
                    {"range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 6}}, "newText": "new_name"}
                ]
            }
        });
        let result = parse_workspace_edit(resp).unwrap();
        assert_eq!(result.changes.len(), 2);
        assert_eq!(total_edit_count(&result), 3);
    }

    #[test]
    fn test_parse_workspace_edit_document_changes() {
        let resp = json!({
            "documentChanges": [{
                "textDocument": {"uri": "file:///src/main.rs", "version": 1},
                "edits": [
                    {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}, "newText": "hello"}
                ]
            }]
        });
        let result = parse_workspace_edit(resp).unwrap();
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_parse_prepare_rename_null() {
        assert!(parse_prepare_rename(JsonValue::Null).is_none());
    }

    #[test]
    fn test_parse_workspace_edit_null() {
        assert!(parse_workspace_edit(JsonValue::Null).is_none());
    }

    #[test]
    fn test_parse_workspace_edit_empty_changes() {
        let resp = json!({"changes": {}});
        assert!(parse_workspace_edit(resp).is_none());
    }
}
