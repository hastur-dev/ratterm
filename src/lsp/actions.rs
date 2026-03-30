//! Code actions (quick fixes, refactorings).

use serde_json::Value as JsonValue;

/// A code action from the server.
#[derive(Debug, Clone)]
pub struct CodeActionResult {
    /// Display title.
    pub title: String,
    /// Kind (e.g., "quickfix", "refactor").
    pub kind: Option<String>,
    /// Whether this is preferred.
    pub is_preferred: bool,
    /// Workspace edit to apply (if any).
    pub edit: Option<super::rename::WorkspaceEditResult>,
    /// Command to execute (if any).
    pub command: Option<CommandInfo>,
    /// Raw JSON for sending back to server.
    pub raw: JsonValue,
}

/// A command to execute.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub title: String,
    pub command: String,
    pub arguments: Vec<JsonValue>,
}

/// Parses a code action response.
pub fn parse_code_actions(result: JsonValue) -> Vec<CodeActionResult> {
    if result.is_null() {
        return Vec::new();
    }

    let Some(arr) = result.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            // Can be CodeAction or Command
            let title = item.get("title")?.as_str()?;
            let kind = item
                .get("kind")
                .and_then(|k| k.as_str())
                .map(|s| s.to_string());
            let is_preferred = item
                .get("isPreferred")
                .and_then(|p| p.as_bool())
                .unwrap_or(false);

            let edit = item
                .get("edit")
                .and_then(|e| super::rename::parse_workspace_edit(e.clone()));

            let command = item.get("command").and_then(|cmd| {
                if cmd.is_object() {
                    Some(CommandInfo {
                        title: cmd
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string(),
                        command: cmd
                            .get("command")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string(),
                        arguments: cmd
                            .get("arguments")
                            .and_then(|a| a.as_array())
                            .cloned()
                            .unwrap_or_default(),
                    })
                } else {
                    None
                }
            });

            Some(CodeActionResult {
                title: title.to_string(),
                kind,
                is_preferred,
                edit,
                command,
                raw: item.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_code_actions() {
        let resp = json!([
            {
                "title": "Add missing import",
                "kind": "quickfix",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [
                            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "use std::io;\n"}
                        ]
                    }
                }
            },
            {
                "title": "Extract to function",
                "kind": "refactor.extract"
            }
        ]);
        let actions = parse_code_actions(resp);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].title, "Add missing import");
        assert!(actions[0].is_preferred);
        assert!(actions[0].edit.is_some());
        assert_eq!(actions[1].title, "Extract to function");
    }

    #[test]
    fn test_parse_code_actions_null() {
        assert!(parse_code_actions(JsonValue::Null).is_empty());
    }

    #[test]
    fn test_parse_code_action_with_command() {
        let resp = json!([{
            "title": "Run test",
            "command": {
                "title": "Run",
                "command": "rust-analyzer.runSingle",
                "arguments": [{"label": "test_foo"}]
            }
        }]);
        let actions = parse_code_actions(resp);
        assert_eq!(actions.len(), 1);
        assert!(actions[0].command.is_some());
        let cmd = actions[0].command.as_ref().unwrap();
        assert_eq!(cmd.command, "rust-analyzer.runSingle");
    }

    #[test]
    fn test_parse_code_actions_empty_array() {
        assert!(parse_code_actions(json!([])).is_empty());
    }

    #[test]
    fn test_parse_code_action_no_optional_fields() {
        let resp = json!([{"title": "Simple action"}]);
        let actions = parse_code_actions(resp);
        assert_eq!(actions.len(), 1);
        assert!(actions[0].kind.is_none());
        assert!(!actions[0].is_preferred);
        assert!(actions[0].edit.is_none());
        assert!(actions[0].command.is_none());
    }
}
