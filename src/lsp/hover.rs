//! Hover information from LSP.

use serde_json::Value as JsonValue;

/// Result from a hover request.
#[derive(Debug, Clone)]
pub struct HoverResult {
    /// Hover content as styled text segments.
    pub contents: Vec<HoverContent>,
    /// Optional range the hover applies to.
    pub range: Option<TextRange>,
}

/// A segment of hover content.
#[derive(Debug, Clone)]
pub enum HoverContent {
    /// Plain text.
    Text(String),
    /// Code block with optional language.
    Code {
        language: Option<String>,
        value: String,
    },
    /// Markdown content (rendered as styled text).
    Markdown(String),
}

/// A text range (line/char based).
#[derive(Debug, Clone, Copy)]
pub struct TextRange {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
}

/// Parses a hover response from LSP JSON.
pub fn parse_hover_response(result: JsonValue) -> Option<HoverResult> {
    // Parse according to LSP spec:
    // { contents: MarkedString | MarkedString[] | MarkupContent, range?: Range }
    let contents_val = result.get("contents")?;
    let contents = parse_hover_contents(contents_val);

    let range = result.get("range").and_then(parse_range);

    if contents.is_empty() {
        return None;
    }

    Some(HoverResult { contents, range })
}

fn parse_hover_contents(val: &JsonValue) -> Vec<HoverContent> {
    let mut result = Vec::new();

    if let Some(s) = val.as_str() {
        // MarkedString as plain string
        result.push(HoverContent::Text(s.to_string()));
    } else if val.is_object() {
        if let Some(kind) = val.get("kind").and_then(|k| k.as_str()) {
            // MarkupContent { kind, value }
            let value = val.get("value").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "markdown" => result.push(HoverContent::Markdown(value.to_string())),
                _ => result.push(HoverContent::Text(value.to_string())),
            }
        } else if let Some(lang) = val.get("language").and_then(|l| l.as_str()) {
            // MarkedString { language, value }
            let value = val.get("value").and_then(|v| v.as_str()).unwrap_or("");
            result.push(HoverContent::Code {
                language: Some(lang.to_string()),
                value: value.to_string(),
            });
        }
    } else if let Some(arr) = val.as_array() {
        // Array of MarkedString
        for item in arr {
            result.extend(parse_hover_contents(item));
        }
    }

    result
}

/// Parses an LSP Range JSON object into a `TextRange`.
pub fn parse_range(val: &JsonValue) -> Option<TextRange> {
    let start = val.get("start")?;
    let end = val.get("end")?;
    Some(TextRange {
        start_line: start.get("line")?.as_u64()? as u32,
        start_char: start.get("character")?.as_u64()? as u32,
        end_line: end.get("line")?.as_u64()? as u32,
        end_char: end.get("character")?.as_u64()? as u32,
    })
}

/// Converts hover content to a vector of (text, is_code) pairs for rendering.
pub fn hover_to_styled_lines(hover: &HoverResult) -> Vec<(String, bool)> {
    let mut lines = Vec::new();
    for content in &hover.contents {
        match content {
            HoverContent::Text(t) => {
                for line in t.lines() {
                    lines.push((line.to_string(), false));
                }
            }
            HoverContent::Code { value, .. } => {
                for line in value.lines() {
                    lines.push((line.to_string(), true));
                }
            }
            HoverContent::Markdown(md) => {
                // Simple markdown rendering: detect code blocks
                let mut in_code_block = false;
                for line in md.lines() {
                    if line.starts_with("```") {
                        in_code_block = !in_code_block;
                        continue;
                    }
                    lines.push((line.to_string(), in_code_block));
                }
            }
        }
    }
    lines
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_hover_plain_string() {
        let resp = json!({"contents": "fn main()"});
        let result = parse_hover_response(resp);
        assert!(result.is_some());
        let hover = result.unwrap();
        assert_eq!(hover.contents.len(), 1);
        assert!(matches!(&hover.contents[0], HoverContent::Text(t) if t == "fn main()"));
    }

    #[test]
    fn test_parse_hover_markup_content() {
        let resp = json!({
            "contents": {
                "kind": "markdown",
                "value": "```rust\nfn main()\n```\nThe entry point."
            }
        });
        let result = parse_hover_response(resp);
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_hover_with_range() {
        let resp = json!({
            "contents": {"kind": "plaintext", "value": "test"},
            "range": {
                "start": {"line": 5, "character": 10},
                "end": {"line": 5, "character": 14}
            }
        });
        let result = parse_hover_response(resp);
        assert!(result.is_some());
        let range = result.unwrap().range.unwrap();
        assert_eq!(range.start_line, 5);
        assert_eq!(range.start_char, 10);
    }

    #[test]
    fn test_hover_to_styled_lines() {
        let hover = HoverResult {
            contents: vec![
                HoverContent::Code {
                    language: Some("rust".into()),
                    value: "fn main()".into(),
                },
                HoverContent::Text("The entry point.".into()),
            ],
            range: None,
        };
        let lines = hover_to_styled_lines(&hover);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].1); // code
        assert!(!lines[1].1); // not code
    }

    #[test]
    fn test_parse_hover_null_returns_none() {
        let resp = json!({});
        assert!(parse_hover_response(resp).is_none());
    }

    #[test]
    fn test_parse_range_valid() {
        let val = json!({"start": {"line": 1, "character": 2}, "end": {"line": 3, "character": 4}});
        let range = parse_range(&val).unwrap();
        assert_eq!(range.start_line, 1);
        assert_eq!(range.start_char, 2);
        assert_eq!(range.end_line, 3);
        assert_eq!(range.end_char, 4);
    }

    #[test]
    fn test_parse_range_missing_fields() {
        let val = json!({"start": {"line": 1}});
        assert!(parse_range(&val).is_none());
    }

    #[test]
    fn test_parse_hover_array_of_marked_strings() {
        let resp = json!({
            "contents": [
                "plain text",
                {"language": "rust", "value": "fn foo()"}
            ]
        });
        let result = parse_hover_response(resp).unwrap();
        assert_eq!(result.contents.len(), 2);
        assert!(matches!(&result.contents[0], HoverContent::Text(_)));
        assert!(matches!(&result.contents[1], HoverContent::Code { .. }));
    }

    #[test]
    fn test_hover_markdown_code_blocks() {
        let hover = HoverResult {
            contents: vec![HoverContent::Markdown(
                "Some text\n```rust\nlet x = 1;\n```\nMore text".into(),
            )],
            range: None,
        };
        let lines = hover_to_styled_lines(&hover);
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].1); // "Some text" - not code
        assert!(lines[1].1); // "let x = 1;" - code
        assert!(!lines[2].1); // "More text" - not code
    }
}
