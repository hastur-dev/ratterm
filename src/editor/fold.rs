//! Code folding: where the foldable regions are, and which are collapsed.
//!
//! [`compute_folds`] derives the regions from the text — braces where the
//! language has them, indentation where it does not, plus runs of line comments.
//! [`FoldState`] owns which of those regions the user has collapsed and answers
//! the two questions a renderer asks: is this line hidden, and which lines do I
//! draw.

use std::collections::HashSet;

use super::brackets::{CharContext, line_contexts};
use super::buffer::Buffer;
use super::highlight::Language;

/// Lines examined by [`compute_folds`] before it stops.
pub const MAX_FOLD_LINES: usize = 100_000;

/// Why a region is foldable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoldKind {
    /// A brace-delimited block.
    Braces,
    /// A block held together by indentation.
    Indent,
    /// A run of consecutive line comments.
    Comment,
}

/// A foldable region.
///
/// `start_line` stays visible when the region is collapsed; the lines from
/// `start_line + 1` through `end_line` are the ones that hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FoldRange {
    /// First line of the region, always visible.
    pub start_line: usize,
    /// Last line of the region.
    pub end_line: usize,
    /// Why the region is foldable.
    pub kind: FoldKind,
}

impl FoldRange {
    /// Returns true when `line` is hidden by collapsing this region.
    #[must_use]
    pub const fn hides(&self, line: usize) -> bool {
        line > self.start_line && line <= self.end_line
    }

    /// Returns true when `line` lies anywhere in the region.
    #[must_use]
    pub const fn contains(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Finds every foldable region in the buffer.
///
/// Regions are returned sorted by start line, with outer regions before the
/// inner ones that share a start.
#[must_use]
pub fn compute_folds(buffer: &Buffer, language: Language) -> Vec<FoldRange> {
    let limit = buffer.len_lines().min(MAX_FOLD_LINES);
    let mut folds = if language.uses_braces() {
        brace_folds(buffer, language, limit)
    } else {
        indent_folds(buffer, limit)
    };
    folds.extend(comment_folds(buffer, language, limit));
    folds.sort_by_key(|f| (f.start_line, std::cmp::Reverse(f.end_line)));
    folds.dedup();
    folds
}

/// Returns a line without its trailing newline.
fn line_text(buffer: &Buffer, line: usize) -> String {
    buffer.line(line).map_or_else(String::new, |raw| {
        raw.strip_suffix('\n').unwrap_or(&raw).to_string()
    })
}

/// Pairs `{` with `}` across the file, ignoring braces in strings and comments.
fn brace_folds(buffer: &Buffer, language: Language, limit: usize) -> Vec<FoldRange> {
    let mut stack: Vec<usize> = Vec::new();
    let mut folds = Vec::new();

    for line in 0..limit {
        let text = line_text(buffer, line);
        let contexts = line_contexts(&text, language);
        for (col, ch) in text.chars().enumerate() {
            if contexts.get(col).copied() != Some(CharContext::Code) {
                continue;
            }
            if ch == '{' {
                stack.push(line);
            } else if ch == '}'
                && let Some(start) = stack.pop()
                && line > start
            {
                folds.push(FoldRange {
                    start_line: start,
                    end_line: line,
                    kind: FoldKind::Braces,
                });
            }
        }
    }
    folds
}

/// Returns the indent width of a line, or `None` when the line is blank.
fn indent_width(text: &str) -> Option<usize> {
    if text.trim().is_empty() {
        return None;
    }
    Some(text.chars().take_while(|c| *c == ' ' || *c == '\t').count())
}

/// Builds folds from indentation: a line whose successor is more deeply
/// indented opens a region that ends at the last line still inside it.
fn indent_folds(buffer: &Buffer, limit: usize) -> Vec<FoldRange> {
    let widths: Vec<Option<usize>> = (0..limit)
        .map(|line| indent_width(&line_text(buffer, line)))
        .collect();

    let mut folds = Vec::new();
    for (start, start_entry) in widths.iter().enumerate() {
        let Some(start_width) = *start_entry else {
            continue;
        };
        let Some(next) = (start + 1..limit).find(|l| widths[*l].is_some()) else {
            continue;
        };
        let Some(next_width) = widths[next] else {
            continue;
        };
        if next_width <= start_width {
            continue;
        }
        let mut end = next;
        for (line, width) in widths.iter().enumerate().skip(next) {
            match width {
                Some(w) if *w > start_width => end = line,
                Some(_) => break,
                None => {}
            }
        }
        folds.push(FoldRange {
            start_line: start,
            end_line: end,
            kind: FoldKind::Indent,
        });
    }
    folds
}

/// Groups runs of two or more consecutive whole-line comments.
fn comment_folds(buffer: &Buffer, language: Language, limit: usize) -> Vec<FoldRange> {
    let Some(marker) = language.line_comment() else {
        return Vec::new();
    };
    let mut folds = Vec::new();
    let mut run_start: Option<usize> = None;

    for line in 0..=limit {
        let is_comment = line < limit && line_text(buffer, line).trim_start().starts_with(marker);
        match (is_comment, run_start) {
            (true, None) => run_start = Some(line),
            (false, Some(start)) => {
                if line - 1 > start {
                    folds.push(FoldRange {
                        start_line: start,
                        end_line: line - 1,
                        kind: FoldKind::Comment,
                    });
                }
                run_start = None;
            }
            _ => {}
        }
    }
    folds
}

/// Which fold regions exist and which of them are collapsed.
///
/// Collapsed state is keyed by a region's start line, so a region keeps its own
/// state while an enclosing region is collapsed over it: expanding the outer
/// region reveals the inner one still folded.
#[derive(Debug, Clone, Default)]
pub struct FoldState {
    ranges: Vec<FoldRange>,
    collapsed: HashSet<usize>,
}

impl FoldState {
    /// Creates state over a set of regions with nothing collapsed.
    #[must_use]
    pub fn new(ranges: Vec<FoldRange>) -> Self {
        Self {
            ranges,
            collapsed: HashSet::new(),
        }
    }

    /// Replaces the regions, keeping collapsed start lines that still exist.
    pub fn set_ranges(&mut self, ranges: Vec<FoldRange>) {
        let starts: HashSet<usize> = ranges.iter().map(|r| r.start_line).collect();
        self.collapsed.retain(|line| starts.contains(line));
        self.ranges = ranges;
    }

    /// Returns the known regions.
    #[must_use]
    pub fn ranges(&self) -> &[FoldRange] {
        &self.ranges
    }

    /// Returns true when a region starting at `line` is collapsed.
    #[must_use]
    pub fn is_collapsed(&self, line: usize) -> bool {
        self.collapsed.contains(&line)
    }

    /// Returns true when anything at all is collapsed.
    ///
    /// The renderer and the edit path both use this to skip work: with nothing
    /// folded, no line can be hidden.
    #[must_use]
    pub fn any_collapsed(&self) -> bool {
        !self.collapsed.is_empty()
    }

    /// Returns the innermost region that `toggle` would act on.
    ///
    /// A region starting exactly at `line` wins; otherwise the smallest region
    /// containing `line` is used.
    #[must_use]
    pub fn region_at(&self, line: usize) -> Option<&FoldRange> {
        self.ranges
            .iter()
            .filter(|r| r.start_line == line)
            .min_by_key(|r| r.end_line - r.start_line)
            .or_else(|| {
                self.ranges
                    .iter()
                    .filter(|r| r.contains(line))
                    .min_by_key(|r| r.end_line - r.start_line)
            })
    }

    /// Collapses or expands the region at `line`.
    ///
    /// Returns false when no region applies, leaving the state untouched.
    pub fn toggle(&mut self, line: usize) -> bool {
        let Some(start) = self.region_at(line).map(|r| r.start_line) else {
            return false;
        };
        if !self.collapsed.remove(&start) {
            self.collapsed.insert(start);
        }
        true
    }

    /// Collapses every region.
    pub fn fold_all(&mut self) {
        self.collapsed = self.ranges.iter().map(|r| r.start_line).collect();
    }

    /// Expands every region.
    pub fn unfold_all(&mut self) {
        self.collapsed.clear();
    }

    /// Returns true when `line` is hidden by some collapsed region.
    #[must_use]
    pub fn is_hidden(&self, line: usize) -> bool {
        if self.collapsed.is_empty() {
            return false;
        }
        self.ranges
            .iter()
            .any(|r| self.collapsed.contains(&r.start_line) && r.hides(line))
    }

    /// Returns the lines a renderer should draw, in order.
    #[must_use]
    pub fn visible_lines(&self, total: usize) -> Vec<usize> {
        let mut hidden = vec![false; total];
        for range in &self.ranges {
            if !self.collapsed.contains(&range.start_line) {
                continue;
            }
            let first = range.start_line + 1;
            let last = range.end_line.min(total.saturating_sub(1));
            if first > last {
                continue;
            }
            for slot in &mut hidden[first..=last] {
                *slot = true;
            }
        }
        (0..total).filter(|line| !hidden[*line]).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn starts_ends(folds: &[FoldRange]) -> Vec<(usize, usize)> {
        folds.iter().map(|f| (f.start_line, f.end_line)).collect()
    }

    #[test]
    fn braces_produce_nested_folds() {
        let src = "fn f() {\n    if a {\n        g();\n    }\n}\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Rust);
        assert_eq!(starts_ends(&folds), vec![(0, 4), (1, 3)]);
        assert!(folds.iter().all(|f| f.kind == FoldKind::Braces));
    }

    #[test]
    fn a_brace_pair_on_one_line_is_not_foldable() {
        let folds = compute_folds(&Buffer::from_str("fn f() {}\n"), Language::Rust);
        assert!(folds.is_empty());
    }

    #[test]
    fn braces_in_strings_and_comments_are_ignored() {
        let src = "fn f() {\n    let s = \"{\";\n    // }\n}\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Rust);
        assert_eq!(starts_ends(&folds), vec![(0, 3)]);
    }

    #[test]
    fn python_folds_on_indentation() {
        let src = "def f():\n    a = 1\n    b = 2\nc = 3\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Python);
        assert_eq!(starts_ends(&folds), vec![(0, 2)]);
        assert_eq!(folds[0].kind, FoldKind::Indent);
    }

    #[test]
    fn python_indent_folds_nest() {
        let src = "def f():\n    if a:\n        b()\n        c()\n    d()\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Python);
        assert_eq!(starts_ends(&folds), vec![(0, 4), (1, 3)]);
    }

    #[test]
    fn blank_lines_inside_a_python_block_stay_folded() {
        let src = "def f():\n    a = 1\n\n    b = 2\nc = 3\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Python);
        assert_eq!(starts_ends(&folds), vec![(0, 3)]);
    }

    #[test]
    fn consecutive_comments_fold() {
        let src = "// one\n// two\n// three\nfn f() {}\n";
        let folds = compute_folds(&Buffer::from_str(src), Language::Rust);
        assert_eq!(starts_ends(&folds), vec![(0, 2)]);
        assert_eq!(folds[0].kind, FoldKind::Comment);
    }

    #[test]
    fn a_single_comment_line_does_not_fold() {
        let folds = compute_folds(&Buffer::from_str("// one\nfn f() {}\n"), Language::Rust);
        assert!(folds.is_empty());
    }

    #[test]
    fn plain_text_has_no_comment_folds() {
        let folds = compute_folds(&Buffer::from_str("// a\n// b\n"), Language::PlainText);
        assert!(folds.iter().all(|f| f.kind != FoldKind::Comment));
    }

    #[test]
    fn collapsing_an_outer_range_hides_the_inner_lines() {
        let src = "fn f() {\n    if a {\n        g();\n    }\n}\n";
        let buffer = Buffer::from_str(src);
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));

        assert!(state.toggle(0));
        assert_eq!(state.visible_lines(6), vec![0, 5]);
        assert!(state.is_hidden(1));
        assert!(!state.is_hidden(0));
    }

    #[test]
    fn an_inner_fold_keeps_its_state_while_the_outer_one_is_collapsed() {
        let src = "fn f() {\n    if a {\n        g();\n    }\n}\n";
        let buffer = Buffer::from_str(src);
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));

        assert!(state.toggle(1));
        assert_eq!(state.visible_lines(6), vec![0, 1, 4, 5]);

        assert!(state.toggle(0));
        assert_eq!(state.visible_lines(6), vec![0, 5]);

        // Expanding the outer fold leaves the inner one collapsed.
        assert!(state.toggle(0));
        assert_eq!(state.visible_lines(6), vec![0, 1, 4, 5]);
    }

    #[test]
    fn toggle_is_its_own_inverse() {
        let buffer = Buffer::from_str("fn f() {\n    g();\n}\n");
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));
        let before = state.visible_lines(4);
        state.toggle(0);
        state.toggle(0);
        assert_eq!(state.visible_lines(4), before);
        assert!(!state.is_collapsed(0));
    }

    #[test]
    fn toggling_a_line_with_no_region_reports_false() {
        let buffer = Buffer::from_str("let a = 1;\n");
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));
        assert!(!state.toggle(0));
        assert_eq!(state.visible_lines(2), vec![0, 1]);
    }

    #[test]
    fn toggling_inside_a_region_folds_the_innermost_one() {
        let src = "fn f() {\n    if a {\n        g();\n    }\n}\n";
        let buffer = Buffer::from_str(src);
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));
        assert!(state.toggle(2));
        assert!(state.is_collapsed(1));
        assert_eq!(state.visible_lines(6), vec![0, 1, 4, 5]);
    }

    #[test]
    fn fold_all_and_unfold_all() {
        let src = "fn f() {\n    if a {\n        g();\n    }\n}\n";
        let buffer = Buffer::from_str(src);
        let mut state = FoldState::new(compute_folds(&buffer, Language::Rust));

        state.fold_all();
        assert_eq!(state.visible_lines(6), vec![0, 5]);
        state.unfold_all();
        assert_eq!(state.visible_lines(6), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn set_ranges_drops_state_for_regions_that_vanished() {
        let mut state = FoldState::new(vec![FoldRange {
            start_line: 3,
            end_line: 6,
            kind: FoldKind::Braces,
        }]);
        state.toggle(3);
        assert!(state.is_collapsed(3));

        state.set_ranges(vec![FoldRange {
            start_line: 9,
            end_line: 12,
            kind: FoldKind::Braces,
        }]);
        assert!(!state.is_collapsed(3));
        assert_eq!(state.visible_lines(13).len(), 13);
    }

    #[test]
    fn visible_lines_clamps_ranges_past_the_end() {
        let state = {
            let mut s = FoldState::new(vec![FoldRange {
                start_line: 0,
                end_line: 99,
                kind: FoldKind::Braces,
            }]);
            s.toggle(0);
            s
        };
        assert_eq!(state.visible_lines(3), vec![0]);
        assert!(state.visible_lines(0).is_empty());
    }
}
