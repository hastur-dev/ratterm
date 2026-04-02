//! Git gutter indicator computation.
//!
//! Converts diff hunks into per-line gutter marks for the editor.

use std::collections::HashMap;

use super::api::DiffResult;

/// Visual mark for a line in the editor gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterMark {
    /// Line was added (green `+`).
    Added,
    /// Line was modified (yellow `~`).
    Modified,
    /// Line was deleted (red `-`) — marks the line *after* the deletion.
    Deleted,
}

/// Computes per-line gutter indicators from a diff result.
///
/// Returns a map of 0-based line number -> `GutterMark`.
pub fn compute_gutter_indicators(diff: &DiffResult) -> HashMap<usize, GutterMark> {
    assert!(
        diff.hunks.len() < 100_000,
        "unreasonable number of hunks"
    );

    let mut marks: HashMap<usize, GutterMark> = HashMap::new();

    for hunk in &diff.hunks {
        let mut has_deletions = false;
        let mut last_new_lineno: Option<usize> = None;

        for line in &hunk.lines {
            match line.origin {
                '+' => {
                    if let Some(n) = line.new_lineno {
                        let line_idx = (n as usize).saturating_sub(1);
                        // If there were deletions in this hunk before additions,
                        // it's a modification, not a pure add.
                        let mark = if has_deletions {
                            GutterMark::Modified
                        } else {
                            GutterMark::Added
                        };
                        marks.insert(line_idx, mark);
                        last_new_lineno = Some(line_idx);
                    }
                }
                '-' => {
                    has_deletions = true;
                    if let Some(n) = line.old_lineno {
                        last_new_lineno = Some((n as usize).saturating_sub(1));
                    }
                }
                _ => {
                    // Context line resets deletion tracking per region
                    if let Some(n) = line.new_lineno {
                        last_new_lineno = Some((n as usize).saturating_sub(1));
                    }
                }
            }
        }

        // If the hunk only had deletions (no additions), mark the line
        // after the deletion point.
        if has_deletions && !hunk.lines.iter().any(|l| l.origin == '+') {
            if let Some(line_idx) = last_new_lineno {
                marks.entry(line_idx).or_insert(GutterMark::Deleted);
            }
        }
    }

    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::api::{DiffHunk, DiffLine, DiffResult};

    fn make_line(origin: char, old: Option<u32>, new: Option<u32>, content: &str) -> DiffLine {
        DiffLine {
            origin,
            content: content.to_string(),
            old_lineno: old,
            new_lineno: new,
        }
    }

    #[test]
    fn test_empty_diff_returns_empty_map() {
        let diff = DiffResult {
            file_path: None,
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
        };
        let marks = compute_gutter_indicators(&diff);
        assert!(marks.is_empty());
    }

    #[test]
    fn test_pure_additions() {
        let diff = DiffResult {
            file_path: None,
            hunks: vec![DiffHunk {
                header: String::new(),
                old_start: 1,
                new_start: 1,
                lines: vec![
                    make_line('+', None, Some(1), "new line 1\n"),
                    make_line('+', None, Some(2), "new line 2\n"),
                ],
            }],
            additions: 2,
            deletions: 0,
        };

        let marks = compute_gutter_indicators(&diff);
        assert_eq!(marks.get(&0), Some(&GutterMark::Added));
        assert_eq!(marks.get(&1), Some(&GutterMark::Added));
        assert_eq!(marks.len(), 2);
    }

    #[test]
    fn test_modification_shows_as_modified() {
        let diff = DiffResult {
            file_path: None,
            hunks: vec![DiffHunk {
                header: String::new(),
                old_start: 3,
                new_start: 3,
                lines: vec![
                    make_line('-', Some(3), None, "old line\n"),
                    make_line('+', None, Some(3), "new line\n"),
                ],
            }],
            additions: 1,
            deletions: 1,
        };

        let marks = compute_gutter_indicators(&diff);
        assert_eq!(marks.get(&2), Some(&GutterMark::Modified));
    }

    #[test]
    fn test_pure_deletion() {
        let diff = DiffResult {
            file_path: None,
            hunks: vec![DiffHunk {
                header: String::new(),
                old_start: 5,
                new_start: 5,
                lines: vec![make_line('-', Some(5), None, "deleted line\n")],
            }],
            additions: 0,
            deletions: 1,
        };

        let marks = compute_gutter_indicators(&diff);
        assert_eq!(marks.get(&4), Some(&GutterMark::Deleted));
    }
}
