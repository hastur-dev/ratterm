//! Go-to-definition, type definition, and implementation.

use std::path::PathBuf;
use serde_json::Value as JsonValue;

/// A location result from definition/references requests.
#[derive(Debug, Clone)]
pub struct LocationResult {
    /// File path (extracted from URI).
    pub path: PathBuf,
    /// Line number (0-based).
    pub line: u32,
    /// Character offset (0-based).
    pub character: u32,
}

/// Parses a definition/typeDefinition/implementation response.
/// Response can be: Location | Location[] | LocationLink[] | null
pub fn parse_location_response(result: JsonValue) -> Vec<LocationResult> {
    if result.is_null() {
        return Vec::new();
    }

    if result.is_array() {
        let arr = result.as_array().unwrap_or(&Vec::new()).clone();
        arr.iter().filter_map(parse_single_location).collect()
    } else if result.is_object() {
        parse_single_location(&result).into_iter().collect()
    } else {
        Vec::new()
    }
}

fn parse_single_location(val: &JsonValue) -> Option<LocationResult> {
    // Try Location format: { uri, range }
    if let Some(uri) = val.get("uri").and_then(|u| u.as_str()) {
        let range = val.get("range")?;
        let start = range.get("start")?;
        let line = start.get("line")?.as_u64()? as u32;
        let character = start.get("character")?.as_u64()? as u32;
        let path = uri_to_path(uri)?;
        return Some(LocationResult {
            path,
            line,
            character,
        });
    }

    // Try LocationLink format: { targetUri, targetRange, targetSelectionRange }
    if let Some(uri) = val.get("targetUri").and_then(|u| u.as_str()) {
        let range = val
            .get("targetSelectionRange")
            .or_else(|| val.get("targetRange"))?;
        let start = range.get("start")?;
        let line = start.get("line")?.as_u64()? as u32;
        let character = start.get("character")?.as_u64()? as u32;
        let path = uri_to_path(uri)?;
        return Some(LocationResult {
            path,
            line,
            character,
        });
    }

    None
}

/// Converts a file URI to a path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let path_str = uri.strip_prefix("file://")?;
    // On Windows, strip leading / from /C:/path
    #[cfg(windows)]
    let path_str = path_str.strip_prefix('/').unwrap_or(path_str);

    // URL-decode percent-encoded characters
    let decoded = percent_decode(path_str);
    Some(PathBuf::from(decoded))
}

/// Simple percent-decoding for file URIs.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Converts a path to a file URI.
pub fn path_to_uri(path: &std::path::Path) -> String {
    let path_str = path.display().to_string().replace('\\', "/");
    if path_str.starts_with('/') {
        format!("file://{path_str}")
    } else {
        format!("file:///{path_str}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_single_location() {
        let resp = json!({
            "uri": "file:///home/user/src/main.rs",
            "range": {
                "start": {"line": 10, "character": 5},
                "end": {"line": 10, "character": 15}
            }
        });
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 10);
        assert_eq!(locations[0].character, 5);
    }

    #[test]
    fn test_parse_location_array() {
        let resp = json!([
            {
                "uri": "file:///src/a.rs",
                "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}
            },
            {
                "uri": "file:///src/b.rs",
                "range": {"start": {"line": 20, "character": 3}, "end": {"line": 20, "character": 10}}
            }
        ]);
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 2);
    }

    #[test]
    fn test_parse_location_link() {
        let resp = json!([{
            "targetUri": "file:///src/lib.rs",
            "targetRange": {"start": {"line": 5, "character": 0}, "end": {"line": 10, "character": 0}},
            "targetSelectionRange": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 8}}
        }]);
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 5);
        assert_eq!(locations[0].character, 4);
    }

    #[test]
    fn test_parse_null() {
        let locations = parse_location_response(JsonValue::Null);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_uri_to_path_unix() {
        #[cfg(not(windows))]
        {
            let path = uri_to_path("file:///home/user/src/main.rs").unwrap();
            assert_eq!(path, PathBuf::from("/home/user/src/main.rs"));
        }
    }

    #[test]
    fn test_uri_to_path_windows() {
        #[cfg(windows)]
        {
            let path = uri_to_path("file:///C:/Users/test/main.rs").unwrap();
            assert_eq!(path, PathBuf::from("C:/Users/test/main.rs"));
        }
    }

    #[test]
    fn test_path_to_uri() {
        #[cfg(not(windows))]
        assert_eq!(
            path_to_uri(std::path::Path::new("/src/main.rs")),
            "file:///src/main.rs"
        );
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("no%25encoding"), "no%encoding");
    }

    #[test]
    fn test_parse_empty_object() {
        let locations = parse_location_response(json!({}));
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_location_link_without_selection_range() {
        let resp = json!([{
            "targetUri": "file:///src/lib.rs",
            "targetRange": {"start": {"line": 5, "character": 0}, "end": {"line": 10, "character": 0}}
        }]);
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 5);
        assert_eq!(locations[0].character, 0);
    }
}
