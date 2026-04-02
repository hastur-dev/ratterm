//! Document formatting.

use super::hover::TextRange;
use serde_json::Value as JsonValue;

/// A text edit from formatting.
#[derive(Debug, Clone)]
pub struct TextEditResult {
    pub range: TextRange,
    pub new_text: String,
}

/// Parses a formatting response (array of text edits).
pub fn parse_formatting_response(result: JsonValue) -> Vec<TextEditResult> {
    if result.is_null() {
        return Vec::new();
    }

    let Some(arr) = result.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|edit| {
            let range = super::hover::parse_range(edit.get("range")?)?;
            let new_text = edit.get("newText")?.as_str()?.to_string();
            Some(TextEditResult { range, new_text })
        })
        .collect()
}

/// Sorts text edits in reverse order (bottom-up, right-to-left) for safe application.
pub fn sort_edits_reverse(edits: &mut [TextEditResult]) {
    edits.sort_by(|a, b| {
        b.range
            .start_line
            .cmp(&a.range.start_line)
            .then(b.range.start_char.cmp(&a.range.start_char))
    });
}

/// Applies text edits to a string buffer.
/// Edits must be sorted in reverse order (call `sort_edits_reverse` first).
pub fn apply_edits_to_string(content: &str, edits: &[TextEditResult]) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = content.to_string();

    // Apply edits in reverse order to preserve positions
    for edit in edits {
        let start_offset =
            line_char_to_offset(&lines, edit.range.start_line, edit.range.start_char);
        let end_offset = line_char_to_offset(&lines, edit.range.end_line, edit.range.end_char);

        if let (Some(start), Some(end)) = (start_offset, end_offset) {
            if start <= end && end <= result.len() {
                result.replace_range(start..end, &edit.new_text);
            }
        }
    }

    result
}

fn line_char_to_offset(lines: &[&str], line: u32, character: u32) -> Option<usize> {
    let line = line as usize;
    if line > lines.len() {
        return None;
    }

    let mut offset = 0;
    for l in lines.iter().take(line) {
        offset += l.len() + 1; // +1 for newline
    }
    offset += character as usize;

    Some(offset)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_formatting_response() {
        let resp = json!([
            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 3}}, "newText": "   "},
            {"range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}}, "newText": "    "}
        ]);
        let edits = parse_formatting_response(resp);
        assert_eq!(edits.len(), 2);
    }

    #[test]
    fn test_parse_formatting_null() {
        assert!(parse_formatting_response(JsonValue::Null).is_empty());
    }

    #[test]
    fn test_sort_edits_reverse() {
        let mut edits = vec![
            TextEditResult {
                range: TextRange {
                    start_line: 1,
                    start_char: 0,
                    end_line: 1,
                    end_char: 5,
                },
                new_text: "a".into(),
            },
            TextEditResult {
                range: TextRange {
                    start_line: 3,
                    start_char: 0,
                    end_line: 3,
                    end_char: 5,
                },
                new_text: "b".into(),
            },
            TextEditResult {
                range: TextRange {
                    start_line: 0,
                    start_char: 0,
                    end_line: 0,
                    end_char: 5,
                },
                new_text: "c".into(),
            },
        ];
        sort_edits_reverse(&mut edits);
        assert_eq!(edits[0].range.start_line, 3);
        assert_eq!(edits[1].range.start_line, 1);
        assert_eq!(edits[2].range.start_line, 0);
    }

    #[test]
    fn test_apply_edits() {
        let content = "hello\nworld\n";
        let edits = vec![TextEditResult {
            range: TextRange {
                start_line: 0,
                start_char: 0,
                end_line: 0,
                end_char: 5,
            },
            new_text: "hi".into(),
        }];
        let result = apply_edits_to_string(content, &edits);
        assert!(result.starts_with("hi"));
    }

    #[test]
    fn test_apply_edits_empty() {
        let content = "hello\n";
        let result = apply_edits_to_string(content, &[]);
        assert_eq!(result, content);
    }

    #[test]
    fn test_parse_formatting_empty_array() {
        assert!(parse_formatting_response(json!([])).is_empty());
    }
}
