//! Indentation detection and adjustment.
//!
//! Every function here works in characters, and every mutating operation is
//! described as a list of [`IndentEdit`] values rather than applied directly, so
//! a caller can preview, batch, or discard them. [`apply_indent_edits`] runs a
//! batch as one undo step.

use std::collections::HashMap;

use super::buffer::{Buffer, Position};
use super::highlight::Language;

/// Indent width used when a file gives no evidence of its own.
pub const DEFAULT_INDENT_WIDTH: usize = 4;

/// Lines examined by [`detect_indent`] before it settles on an answer.
pub const MAX_DETECT_LINES: usize = 5_000;

/// How one indentation level is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    /// A number of spaces per level.
    Spaces(usize),
    /// One tab per level.
    Tabs,
}

impl Default for IndentStyle {
    fn default() -> Self {
        Self::Spaces(DEFAULT_INDENT_WIDTH)
    }
}

impl IndentStyle {
    /// Returns the text of one indentation level.
    #[must_use]
    pub fn unit(self) -> String {
        match self {
            Self::Spaces(n) => " ".repeat(n.max(1)),
            Self::Tabs => "\t".to_string(),
        }
    }

    /// Returns how many columns one level occupies.
    #[must_use]
    pub const fn width(self) -> usize {
        match self {
            Self::Spaces(n) => {
                if n == 0 {
                    1
                } else {
                    n
                }
            }
            Self::Tabs => 1,
        }
    }
}

/// Guesses a file's indentation style from the lines it already has.
///
/// Tabs win when more lines start with a tab than the number of space-indent
/// steps observed. Otherwise the most frequent positive step between the indents
/// of consecutive non-blank lines is the width, with ties going to the narrower
/// step. A file with no indentation gets [`IndentStyle::default`].
#[must_use]
pub fn detect_indent(buffer: &Buffer) -> IndentStyle {
    let mut tab_lines = 0usize;
    let mut steps: HashMap<usize, usize> = HashMap::new();
    let mut previous: Option<usize> = None;

    let limit = buffer.len_lines().min(MAX_DETECT_LINES);
    for line in 0..limit {
        let Some(raw) = buffer.line(line) else {
            break;
        };
        let text = raw.strip_suffix('\n').unwrap_or(&raw);
        if text.trim().is_empty() {
            continue;
        }
        let lead = leading_whitespace(text);
        if lead.starts_with('\t') {
            tab_lines += 1;
            previous = None;
            continue;
        }
        let spaces = lead.chars().count();
        if let Some(prev) = previous
            && spaces > prev
        {
            *steps.entry(spaces - prev).or_insert(0) += 1;
        }
        previous = Some(spaces);
    }

    let space_votes: usize = steps.values().sum();
    if tab_lines > space_votes {
        return IndentStyle::Tabs;
    }
    steps
        .into_iter()
        .max_by_key(|(width, count)| (*count, std::cmp::Reverse(*width)))
        .filter(|(width, _)| *width > 0 && *width <= 16)
        .map_or_else(IndentStyle::default, |(width, _)| {
            IndentStyle::Spaces(width)
        })
}

/// Returns the leading whitespace of a string, verbatim.
fn leading_whitespace(text: &str) -> String {
    text.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// Returns a line's leading whitespace, verbatim.
///
/// An out-of-range line yields an empty string.
#[must_use]
pub fn indent_of_line(buffer: &Buffer, line: usize) -> String {
    buffer.line(line).map_or_else(String::new, |raw| {
        leading_whitespace(raw.strip_suffix('\n').unwrap_or(&raw))
    })
}

/// Removes one indentation level from the end of an indent string.
fn strip_one_level(indent: &str, style: IndentStyle) -> String {
    let mut chars: Vec<char> = indent.chars().collect();
    match style {
        IndentStyle::Tabs => {
            if chars.last() == Some(&'\t') {
                chars.pop();
            } else {
                for _ in 0..DEFAULT_INDENT_WIDTH {
                    if chars.last() == Some(&' ') {
                        chars.pop();
                    } else {
                        break;
                    }
                }
            }
        }
        IndentStyle::Spaces(n) => {
            if chars.last() == Some(&'\t') {
                chars.pop();
            } else {
                for _ in 0..n.max(1) {
                    if chars.last() == Some(&' ') {
                        chars.pop();
                    } else {
                        break;
                    }
                }
            }
        }
    }
    chars.into_iter().collect()
}

/// Removes one indentation level from the front of an indent string.
fn outdent_prefix(indent: &str, style: IndentStyle) -> String {
    let chars: Vec<char> = indent.chars().collect();
    let drop = match (chars.first(), style) {
        (Some('\t'), _) => 1,
        (Some(' '), IndentStyle::Spaces(n)) => chars
            .iter()
            .take(n.max(1))
            .take_while(|c| **c == ' ')
            .count(),
        (Some(' '), IndentStyle::Tabs) => chars
            .iter()
            .take(DEFAULT_INDENT_WIDTH)
            .take_while(|c| **c == ' ')
            .count(),
        _ => 0,
    };
    chars[drop..].iter().collect()
}

/// Returns the indentation a line inserted at `pos` should start with.
///
/// The current line's indent is the base. One level is added after an opening
/// bracket, or after a line ending in `:` in Python. One level is removed when
/// the text that will begin the new line starts with a closing bracket.
#[must_use]
pub fn indent_for_new_line(
    buffer: &Buffer,
    pos: Position,
    style: &IndentStyle,
    language: Language,
) -> String {
    let raw = buffer.line(pos.line).unwrap_or_default();
    let text = raw.strip_suffix('\n').unwrap_or(&raw).to_string();
    let chars: Vec<char> = text.chars().collect();
    let split = pos.col.min(chars.len());
    let before: String = chars[..split].iter().collect();
    let after: String = chars[split..].iter().collect();

    let base = leading_whitespace(&text);
    let head = before.trim_end();
    let tail = after.trim_start();

    if tail.starts_with('}') || tail.starts_with(')') || tail.starts_with(']') {
        return strip_one_level(&base, *style);
    }

    let opens_block = head.ends_with('{') || head.ends_with('(') || head.ends_with('[');
    let opens_suite = language == Language::Python && head.ends_with(':');
    if opens_block || opens_suite {
        return format!("{}{}", base, style.unit());
    }
    base
}

/// A replacement of one line's leading whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentEdit {
    /// Line to change.
    pub line: usize,
    /// Character count of the whitespace being replaced.
    pub old_len: usize,
    /// Whitespace to put in its place.
    pub new_indent: String,
}

/// Describes setting one line's indent to `new_indent`.
///
/// Returns `None` when the line does not exist or already has that indent.
#[must_use]
pub fn reindent_line(buffer: &Buffer, line: usize, new_indent: &str) -> Option<IndentEdit> {
    let raw = buffer.line(line)?;
    let text = raw.strip_suffix('\n').unwrap_or(&raw);
    let current = leading_whitespace(text);
    if current == new_indent {
        return None;
    }
    Some(IndentEdit {
        line,
        old_len: current.chars().count(),
        new_indent: new_indent.to_string(),
    })
}

/// Describes adding one indentation level to every line in `lines`.
///
/// Entirely empty lines are left alone so that indenting and then outdenting a
/// block restores it exactly.
#[must_use]
pub fn indent_lines(
    buffer: &Buffer,
    lines: std::ops::RangeInclusive<usize>,
    style: IndentStyle,
) -> Vec<IndentEdit> {
    let unit = style.unit();
    let mut edits = Vec::new();
    for line in lines {
        if line >= buffer.len_lines() {
            break;
        }
        let Some(raw) = buffer.line(line) else { break };
        let text = raw.strip_suffix('\n').unwrap_or(&raw);
        if text.is_empty() {
            continue;
        }
        let current = leading_whitespace(text);
        edits.push(IndentEdit {
            line,
            old_len: current.chars().count(),
            new_indent: format!("{}{}", unit, current),
        });
    }
    edits
}

/// Describes removing one indentation level from every line in `lines`.
#[must_use]
pub fn outdent_lines(
    buffer: &Buffer,
    lines: std::ops::RangeInclusive<usize>,
    style: IndentStyle,
) -> Vec<IndentEdit> {
    let mut edits = Vec::new();
    for line in lines {
        if line >= buffer.len_lines() {
            break;
        }
        let Some(raw) = buffer.line(line) else { break };
        let text = raw.strip_suffix('\n').unwrap_or(&raw);
        if text.is_empty() {
            continue;
        }
        let current = leading_whitespace(text);
        let shortened = outdent_prefix(&current, style);
        if shortened == current {
            continue;
        }
        edits.push(IndentEdit {
            line,
            old_len: current.chars().count(),
            new_indent: shortened,
        });
    }
    edits
}

/// Applies indent edits to the buffer as one undo step.
///
/// Edits are applied from the last line backwards so earlier line indices stay
/// valid; changing a line's indent never moves another line.
pub fn apply_indent_edits(buffer: &mut Buffer, edits: &[IndentEdit]) {
    if edits.is_empty() {
        return;
    }
    let mut ordered: Vec<&IndentEdit> = edits.iter().collect();
    ordered.sort_by_key(|e| std::cmp::Reverse(e.line));

    buffer.begin_undo_group();
    for edit in ordered {
        if edit.old_len > 0 {
            buffer.delete_range(
                Position::new(edit.line, 0),
                Position::new(edit.line, edit.old_len),
            );
        }
        if !edit.new_indent.is_empty() {
            buffer.insert_str(Position::new(edit.line, 0), &edit.new_indent);
        }
    }
    buffer.end_undo_group();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn detects_four_space_indentation() {
        let buffer =
            Buffer::from_str("fn f() {\n    let a = 1;\n    if a {\n        g();\n    }\n}\n");
        assert_eq!(detect_indent(&buffer), IndentStyle::Spaces(4));
    }

    #[test]
    fn detects_two_space_indentation() {
        let buffer =
            Buffer::from_str("function f() {\n  let a = 1;\n  if (a) {\n    g();\n  }\n}\n");
        assert_eq!(detect_indent(&buffer), IndentStyle::Spaces(2));
    }

    #[test]
    fn detects_tabs() {
        let buffer = Buffer::from_str("fn f() {\n\tlet a = 1;\n\tif a {\n\t\tg();\n\t}\n}\n");
        assert_eq!(detect_indent(&buffer), IndentStyle::Tabs);
    }

    #[test]
    fn a_flat_file_falls_back_to_the_default() {
        let buffer = Buffer::from_str("one\ntwo\nthree\n");
        assert_eq!(detect_indent(&buffer), IndentStyle::Spaces(4));
        assert_eq!(detect_indent(&Buffer::new()), IndentStyle::Spaces(4));
    }

    #[test]
    fn blank_lines_do_not_break_detection() {
        let buffer = Buffer::from_str("a:\n\n  b\n\n    c\n");
        assert_eq!(detect_indent(&buffer), IndentStyle::Spaces(2));
    }

    #[test]
    fn indent_of_line_is_verbatim() {
        let buffer = Buffer::from_str("  \tmixed\nplain\n");
        assert_eq!(indent_of_line(&buffer, 0), "  \t");
        assert_eq!(indent_of_line(&buffer, 1), "");
        assert_eq!(indent_of_line(&buffer, 99), "");
    }

    #[test]
    fn new_line_indents_after_an_opening_brace() {
        let buffer = Buffer::from_str("    fn f() {\n");
        let style = IndentStyle::Spaces(4);
        let got = indent_for_new_line(&buffer, Position::new(0, 12), &style, Language::Rust);
        assert_eq!(got, "        ");
    }

    #[test]
    fn new_line_indents_after_a_python_colon() {
        let buffer = Buffer::from_str("  def f():\n");
        let style = IndentStyle::Spaces(2);
        let got = indent_for_new_line(&buffer, Position::new(0, 10), &style, Language::Python);
        assert_eq!(got, "    ");
        // The same line in Rust is not a block opener.
        let got = indent_for_new_line(&buffer, Position::new(0, 10), &style, Language::Rust);
        assert_eq!(got, "  ");
    }

    #[test]
    fn new_line_outdents_before_a_closing_brace() {
        let buffer = Buffer::from_str("        }\n");
        let style = IndentStyle::Spaces(4);
        let got = indent_for_new_line(&buffer, Position::new(0, 8), &style, Language::Rust);
        assert_eq!(got, "    ");
    }

    #[test]
    fn new_line_copies_the_indent_by_default() {
        let buffer = Buffer::from_str("      let a = 1;\n");
        let style = IndentStyle::Spaces(2);
        let got = indent_for_new_line(&buffer, Position::new(0, 16), &style, Language::Rust);
        assert_eq!(got, "      ");
    }

    #[test]
    fn new_line_indent_uses_tabs_when_that_is_the_style() {
        let buffer = Buffer::from_str("\tfn f() {\n");
        let got = indent_for_new_line(
            &buffer,
            Position::new(0, 9),
            &IndentStyle::Tabs,
            Language::Rust,
        );
        assert_eq!(got, "\t\t");
    }

    #[test]
    fn reindent_line_reports_no_edit_when_unchanged() {
        let buffer = Buffer::from_str("    a\n");
        assert!(reindent_line(&buffer, 0, "    ").is_none());
        assert_eq!(
            reindent_line(&buffer, 0, "  "),
            Some(IndentEdit {
                line: 0,
                old_len: 4,
                new_indent: "  ".to_string()
            })
        );
        assert!(reindent_line(&buffer, 42, "").is_none());
    }

    #[test]
    fn indent_and_outdent_change_the_buffer() {
        let mut buffer = Buffer::from_str("a\n  b\nc\n");
        let edits = indent_lines(&buffer, 0..=2, IndentStyle::Spaces(2));
        apply_indent_edits(&mut buffer, &edits);
        assert_eq!(buffer.text(), "  a\n    b\n  c\n");

        let edits = outdent_lines(&buffer, 0..=2, IndentStyle::Spaces(2));
        apply_indent_edits(&mut buffer, &edits);
        assert_eq!(buffer.text(), "a\n  b\nc\n");
    }

    #[test]
    fn outdent_does_nothing_to_an_unindented_line() {
        let buffer = Buffer::from_str("a\nb\n");
        assert!(outdent_lines(&buffer, 0..=1, IndentStyle::Spaces(4)).is_empty());
    }

    #[test]
    fn indent_skips_empty_lines_and_ranges_past_the_end() {
        let buffer = Buffer::from_str("a\n\nb\n");
        let edits = indent_lines(&buffer, 0..=99, IndentStyle::Tabs);
        assert_eq!(edits.iter().map(|e| e.line).collect::<Vec<_>>(), vec![0, 2]);
    }

    #[test]
    fn applying_a_batch_is_one_undo_step() {
        let mut buffer = Buffer::from_str("a\nb\nc\n");
        let edits = indent_lines(&buffer, 0..=2, IndentStyle::Spaces(4));
        apply_indent_edits(&mut buffer, &edits);
        assert_eq!(buffer.text(), "    a\n    b\n    c\n");
        buffer.undo();
        assert_eq!(buffer.text(), "a\nb\nc\n");
    }

    proptest! {
        #[test]
        fn indent_then_outdent_restores_the_text(
            lines in prop::collection::vec("[ \t]{0,6}[a-z ]{0,12}", 1..8),
            width in 1usize..8,
            tabs in any::<bool>(),
        ) {
            let style = if tabs { IndentStyle::Tabs } else { IndentStyle::Spaces(width) };
            let original = format!("{}\n", lines.join("\n"));
            let mut buffer = Buffer::from_str(&original);
            let last = buffer.len_lines().saturating_sub(1);

            let edits = indent_lines(&buffer, 0..=last, style);
            apply_indent_edits(&mut buffer, &edits);
            let edits = outdent_lines(&buffer, 0..=last, style);
            apply_indent_edits(&mut buffer, &edits);

            prop_assert_eq!(buffer.text(), original);
        }
    }
}
