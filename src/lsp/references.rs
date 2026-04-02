//! Find references functionality.

use super::definition::LocationResult;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// References grouped by file.
#[derive(Debug, Clone)]
pub struct ReferenceGroup {
    /// File path.
    pub path: PathBuf,
    /// References in this file (line, character, optional preview text).
    pub locations: Vec<ReferenceLocation>,
}

/// A single reference location with preview.
#[derive(Debug, Clone)]
pub struct ReferenceLocation {
    pub line: u32,
    pub character: u32,
    pub end_character: u32,
    pub preview: String,
}

/// Parses a references response and groups by file.
pub fn parse_references_response(result: JsonValue) -> Vec<ReferenceGroup> {
    if result.is_null() {
        return Vec::new();
    }

    let locations: Vec<LocationResult> = super::definition::parse_location_response(result);
    group_locations(locations)
}

/// Groups location results by file path.
pub fn group_locations(locations: Vec<LocationResult>) -> Vec<ReferenceGroup> {
    let mut grouped: BTreeMap<PathBuf, Vec<ReferenceLocation>> = BTreeMap::new();

    for loc in locations {
        grouped
            .entry(loc.path.clone())
            .or_default()
            .push(ReferenceLocation {
                line: loc.line,
                character: loc.character,
                end_character: loc.character, // Updated if range end is available
                preview: String::new(),       // Filled in by caller
            });
    }

    grouped
        .into_iter()
        .map(|(path, mut locations)| {
            locations.sort_by_key(|l| (l.line, l.character));
            ReferenceGroup { path, locations }
        })
        .collect()
}

/// Fills in preview text for reference locations by reading file content.
pub fn fill_previews(groups: &mut [ReferenceGroup]) {
    for group in groups.iter_mut() {
        if let Ok(content) = std::fs::read_to_string(&group.path) {
            let file_lines: Vec<&str> = content.lines().collect();
            for loc in &mut group.locations {
                if let Some(line_text) = file_lines.get(loc.line as usize) {
                    loc.preview = line_text.trim().to_string();
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_references_null() {
        let groups = parse_references_response(JsonValue::Null);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_parse_references_groups_by_file() {
        let resp = json!([
            {"uri": "file:///src/a.rs", "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}},
            {"uri": "file:///src/a.rs", "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 5}}},
            {"uri": "file:///src/b.rs", "range": {"start": {"line": 3, "character": 2}, "end": {"line": 3, "character": 8}}}
        ]);
        let groups = parse_references_response(resp);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_group_locations() {
        let locations = vec![
            LocationResult {
                path: PathBuf::from("a.rs"),
                line: 5,
                character: 0,
            },
            LocationResult {
                path: PathBuf::from("a.rs"),
                line: 1,
                character: 3,
            },
            LocationResult {
                path: PathBuf::from("b.rs"),
                line: 10,
                character: 0,
            },
        ];
        let groups = group_locations(locations);
        assert_eq!(groups.len(), 2);
        // First group (a.rs) should be sorted by line
        assert_eq!(groups[0].locations[0].line, 1);
        assert_eq!(groups[0].locations[1].line, 5);
    }

    #[test]
    fn test_group_empty_locations() {
        let groups = group_locations(Vec::new());
        assert!(groups.is_empty());
    }
}
