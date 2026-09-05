//! Tree-sitter syntax highlighting.
//!
//! The grammars shipped with the crate (Rust, Python, JavaScript) each expose a
//! highlight query, so highlighting is driven by those queries rather than by a
//! hand-written node-kind table. Capture names are mapped to [`HighlightKind`],
//! a renderer-agnostic set; the theme layer decides what colour each kind gets.
//!
//! Every column in this module is a character offset within a line, never a
//! byte offset. Tree-sitter reports byte offsets, so [`Highlighter`] converts
//! them against the line text before returning spans.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use thiserror::Error;
use tree_sitter::{
    InputEdit, Language as TsLanguage, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};

use super::buffer::{Buffer, Position};

/// Files at or above this size are not parsed.
///
/// A full reparse of a very large file on every edit costs more than the
/// highlighting is worth, and the editor already opens files this size
/// read-only.
pub const MAX_HIGHLIGHT_BYTES: usize = 2 * 1024 * 1024;

/// A language the editor can highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// Rust.
    Rust,
    /// Python.
    Python,
    /// JavaScript (including JSX-free `.mjs` and `.cjs`).
    JavaScript,
    /// Anything without a grammar. Produces no spans.
    #[default]
    PlainText,
}

impl Language {
    /// Picks a language from a file path's extension.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return Self::PlainText;
        };
        match ext.to_ascii_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" | "pyi" | "pyw" => Self::Python,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            _ => Self::PlainText,
        }
    }

    /// Returns the human-readable name, for status display.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::PlainText => "Plain Text",
        }
    }

    /// Returns the tree-sitter grammar, or `None` for [`Language::PlainText`].
    #[must_use]
    pub fn ts_language(self) -> Option<TsLanguage> {
        match self {
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Self::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Self::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Self::PlainText => None,
        }
    }

    /// Returns the grammar's highlight query source.
    ///
    /// The JavaScript grammar names its constant `HIGHLIGHT_QUERY` while the
    /// other two use `HIGHLIGHTS_QUERY`.
    #[must_use]
    pub const fn highlights_query(self) -> Option<&'static str> {
        match self {
            Self::Rust => Some(tree_sitter_rust::HIGHLIGHTS_QUERY),
            Self::Python => Some(tree_sitter_python::HIGHLIGHTS_QUERY),
            Self::JavaScript => Some(tree_sitter_javascript::HIGHLIGHT_QUERY),
            Self::PlainText => None,
        }
    }

    /// Returns the line-comment marker, when the language has one.
    #[must_use]
    pub const fn line_comment(self) -> Option<&'static str> {
        match self {
            Self::Rust | Self::JavaScript => Some("//"),
            Self::Python => Some("#"),
            Self::PlainText => None,
        }
    }

    /// Returns true when block structure is expressed with braces.
    ///
    /// Folding and new-line indentation use this to choose between brace
    /// tracking and indentation tracking.
    #[must_use]
    pub const fn uses_braces(self) -> bool {
        matches!(self, Self::Rust | Self::JavaScript)
    }

    /// Returns true when `'` opens a string rather than a character literal or
    /// an apostrophe.
    #[must_use]
    pub const fn single_quote_is_string(self) -> bool {
        matches!(self, Self::Python | Self::JavaScript)
    }
}

/// A renderer-agnostic highlight category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    /// Language keyword.
    Keyword,
    /// Function or method name.
    Function,
    /// Type name.
    Type,
    /// String or character literal, including escapes.
    String,
    /// Numeric literal.
    Number,
    /// Comment.
    Comment,
    /// Operator token.
    Operator,
    /// Bracket or delimiter.
    Punctuation,
    /// Variable, parameter, or field.
    Variable,
    /// Named constant.
    Constant,
    /// Attribute, annotation, or decorator.
    Attribute,
}

/// Maps a tree-sitter capture name to a highlight kind.
///
/// Unknown captures return `None` and contribute no span; the full capture name
/// is tried first so that `constant.builtin` can differ from `constant` if the
/// mapping ever needs it.
#[must_use]
pub fn kind_for_capture(name: &str) -> Option<HighlightKind> {
    let head = name.split('.').next().unwrap_or(name);
    match head {
        "keyword" => Some(HighlightKind::Keyword),
        "function" => Some(HighlightKind::Function),
        "type" | "constructor" => Some(HighlightKind::Type),
        "string" | "escape" | "character" => Some(HighlightKind::String),
        "number" | "float" | "integer" => Some(HighlightKind::Number),
        "comment" => Some(HighlightKind::Comment),
        "operator" => Some(HighlightKind::Operator),
        "punctuation" | "label" => Some(HighlightKind::Punctuation),
        "variable" | "property" | "parameter" | "field" => Some(HighlightKind::Variable),
        "constant" => Some(HighlightKind::Constant),
        "attribute" | "annotation" | "decorator" => Some(HighlightKind::Attribute),
        _ => None,
    }
}

/// A highlighted run of characters on one line.
///
/// `start_col` is inclusive and `end_col` exclusive, both character offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    /// First character column covered.
    pub start_col: usize,
    /// One past the last character column covered.
    pub end_col: usize,
    /// What the run is.
    pub kind: HighlightKind,
}

/// Something that stopped a language from being loaded.
#[derive(Debug, Error)]
pub enum HighlightError {
    /// The grammar was rejected by the tree-sitter runtime.
    #[error("cannot load the {language} grammar")]
    Grammar {
        /// Language whose grammar failed.
        language: &'static str,
        /// Underlying tree-sitter error.
        #[source]
        source: tree_sitter::LanguageError,
    },
    /// The grammar's highlight query did not compile.
    #[error("cannot compile the {language} highlight query")]
    Query {
        /// Language whose query failed.
        language: &'static str,
        /// Underlying tree-sitter error.
        #[source]
        source: tree_sitter::QueryError,
    },
}

/// Incremental syntax highlighter for one document.
///
/// Call [`Highlighter::parse`] once after loading a file, then
/// [`Highlighter::edit`] followed by [`Highlighter::parse`] after each buffer
/// change so tree-sitter can reuse the previous tree. [`Highlighter::highlight_line`]
/// is cheap to call repeatedly: results are cached per line and the cache is
/// dropped whenever the tree changes.
pub struct Highlighter {
    language: Language,
    parser: Option<Parser>,
    query: Option<Query>,
    tree: Option<Tree>,
    source: String,
    cache: RefCell<HashMap<usize, Vec<HighlightSpan>>>,
}

impl fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Highlighter")
            .field("language", &self.language)
            .field("has_tree", &self.tree.is_some())
            .field("source_len", &self.source.len())
            .finish()
    }
}

impl Highlighter {
    /// Creates a highlighter for `language`.
    ///
    /// # Errors
    /// Returns an error if the grammar or its highlight query cannot be loaded.
    pub fn new(language: Language) -> Result<Self, HighlightError> {
        let mut this = Self {
            language: Language::PlainText,
            parser: None,
            query: None,
            tree: None,
            source: String::new(),
            cache: RefCell::new(HashMap::new()),
        };
        this.set_language(language)?;
        Ok(this)
    }

    /// Creates a highlighter for the language implied by `path`.
    ///
    /// # Errors
    /// Same conditions as [`Highlighter::new`].
    pub fn for_path(path: &Path) -> Result<Self, HighlightError> {
        Self::new(Language::from_path(path))
    }

    /// Returns the active language.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Returns true once a tree has been parsed.
    #[must_use]
    pub const fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    /// Returns how many lines are currently cached. Used by tests.
    #[must_use]
    pub fn cached_line_count(&self) -> usize {
        self.cache.borrow().len()
    }

    /// Switches language, discarding the parsed tree and the cache.
    ///
    /// # Errors
    /// Returns an error if the grammar or its highlight query cannot be loaded.
    pub fn set_language(&mut self, language: Language) -> Result<(), HighlightError> {
        self.tree = None;
        self.source.clear();
        self.cache.borrow_mut().clear();
        self.language = language;

        let Some(ts) = language.ts_language() else {
            self.parser = None;
            self.query = None;
            return Ok(());
        };

        let mut parser = Parser::new();
        parser
            .set_language(&ts)
            .map_err(|source| HighlightError::Grammar {
                language: language.name(),
                source,
            })?;

        let query = match language.highlights_query() {
            Some(src) => Some(
                Query::new(&ts, src).map_err(|source| HighlightError::Query {
                    language: language.name(),
                    source,
                })?,
            ),
            None => None,
        };

        self.parser = Some(parser);
        self.query = query;
        Ok(())
    }

    /// Records a buffer change against the current tree.
    ///
    /// Call this before [`Highlighter::parse`] so the reparse can reuse the
    /// unchanged parts of the tree. Use [`insertion_edit`] or [`deletion_edit`]
    /// to build the argument from buffer positions.
    pub fn edit(&mut self, edit: &InputEdit) {
        self.cache.borrow_mut().clear();
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(edit);
        }
    }

    /// Parses `buffer`, reusing the previous tree when one exists.
    ///
    /// Failure is silent by design: a document that cannot be parsed simply has
    /// no spans, so the renderer never has to handle an error.
    pub fn parse(&mut self, buffer: &Buffer) {
        self.cache.borrow_mut().clear();

        let Some(parser) = self.parser.as_mut() else {
            self.tree = None;
            self.source.clear();
            return;
        };

        let text = buffer.text();
        if text.len() >= MAX_HIGHLIGHT_BYTES {
            self.tree = None;
            self.source.clear();
            return;
        }

        self.tree = parser.parse(&text, self.tree.as_ref());
        if self.tree.is_none() {
            tracing::debug!("tree-sitter returned no tree for {}", self.language.name());
        }
        self.source = text;
    }

    /// Returns the highlight spans covering one line.
    ///
    /// `buffer` must be the buffer most recently passed to
    /// [`Highlighter::parse`]. An unparsed document, an unsupported language, or
    /// an out-of-range line all yield an empty vector.
    #[must_use]
    pub fn highlight_line(&self, buffer: &Buffer, line: usize) -> Vec<HighlightSpan> {
        if let Some(hit) = self.cache.borrow().get(&line) {
            return hit.clone();
        }
        let spans = self.compute_line(buffer, line);
        self.cache.borrow_mut().insert(line, spans.clone());
        spans
    }

    /// Walks the query captures overlapping one line and paints them.
    fn compute_line(&self, buffer: &Buffer, line: usize) -> Vec<HighlightSpan> {
        let (Some(tree), Some(query)) = (self.tree.as_ref(), self.query.as_ref()) else {
            return Vec::new();
        };
        let Some(raw) = buffer.line(line) else {
            return Vec::new();
        };
        let text = raw.strip_suffix('\n').unwrap_or(&raw);
        let byte_len = text.len();
        let byte_to_char = byte_to_char_map(text);
        let char_len = byte_to_char.last().copied().unwrap_or(0);
        if char_len == 0 {
            return Vec::new();
        }

        let names = query.capture_names();
        let mut cursor = QueryCursor::new();
        cursor.set_point_range(Point::new(line, 0)..Point::new(line + 1, 0));

        // (node byte length, encounter order, start char, end char, kind)
        let mut painted: Vec<(usize, usize, usize, usize, HighlightKind)> = Vec::new();
        let mut order = 0usize;
        let source = self.source.as_bytes();
        let mut matches = cursor.matches(query, tree.root_node(), source);
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let Some(name) = names.get(capture.index as usize) else {
                    continue;
                };
                let Some(kind) = kind_for_capture(name) else {
                    continue;
                };
                let start = capture.node.start_position();
                let end = capture.node.end_position();
                if end.row < line || start.row > line {
                    continue;
                }
                let start_byte = if start.row == line { start.column } else { 0 };
                let end_byte = if end.row == line {
                    end.column
                } else {
                    byte_len
                };
                let start_col = byte_to_char
                    .get(start_byte.min(byte_len))
                    .copied()
                    .unwrap_or(0);
                let end_col = byte_to_char
                    .get(end_byte.min(byte_len))
                    .copied()
                    .unwrap_or(char_len);
                if start_col >= end_col {
                    continue;
                }
                let size = capture
                    .node
                    .end_byte()
                    .saturating_sub(capture.node.start_byte());
                painted.push((size, order, start_col, end_col, kind));
                order += 1;
            }
        }

        // Paint widest first so nested, more specific captures win. Equal-width
        // captures are painted newest first so the earliest query pattern, which
        // tree-sitter treats as the higher priority one, lands last.
        painted
            .sort_by_key(|(size, order, ..)| (std::cmp::Reverse(*size), std::cmp::Reverse(*order)));

        let mut kinds: Vec<Option<HighlightKind>> = vec![None; char_len];
        for (_, _, start_col, end_col, kind) in painted {
            for slot in &mut kinds[start_col..end_col.min(char_len)] {
                *slot = Some(kind);
            }
        }

        coalesce(&kinds)
    }
}

/// Merges a per-character kind array into runs.
fn coalesce(kinds: &[Option<HighlightKind>]) -> Vec<HighlightSpan> {
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < kinds.len() {
        let Some(kind) = kinds[i] else {
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < kinds.len() && kinds[j] == Some(kind) {
            j += 1;
        }
        spans.push(HighlightSpan {
            start_col: i,
            end_col: j,
            kind,
        });
        i = j;
    }
    spans
}

/// Builds a byte-offset to character-offset map for one line.
///
/// The returned vector has `text.len() + 1` entries so the end offset maps to
/// the line's character count.
fn byte_to_char_map(text: &str) -> Vec<usize> {
    let mut map = vec![0usize; text.len() + 1];
    let mut chars = 0usize;
    for (byte, ch) in text.char_indices() {
        for slot in map.iter_mut().skip(byte).take(ch.len_utf8()) {
            *slot = chars;
        }
        chars += 1;
    }
    if let Some(last) = map.last_mut() {
        *last = chars;
    }
    map
}

/// Returns the byte offset and tree-sitter point for a buffer position.
///
/// `Point::column` is a byte offset within its line, which is why this cannot
/// reuse the character columns carried by [`Position`].
fn locate(text: &str, pos: Position) -> (usize, Point) {
    let bytes = text.as_bytes();
    let mut line_start = 0usize;
    let mut row = 0usize;
    let mut i = 0usize;
    while i < bytes.len() && row < pos.line {
        if bytes[i] == b'\n' {
            row += 1;
            line_start = i + 1;
        }
        i += 1;
    }

    let mut byte = line_start;
    for (cols, ch) in text[line_start..].chars().enumerate() {
        if cols >= pos.col || ch == '\n' {
            break;
        }
        byte += ch.len_utf8();
    }
    (byte, Point::new(row, byte - line_start))
}

/// Builds the tree-sitter edit describing an insertion into `before`.
///
/// `before` is the buffer as it was prior to the insertion.
#[must_use]
pub fn insertion_edit(before: &Buffer, at: Position, inserted: &str) -> InputEdit {
    let text = before.text();
    let (start_byte, start_position) = locate(&text, at);
    let added_lines = inserted.matches('\n').count();
    let new_end_position = if added_lines == 0 {
        Point::new(start_position.row, start_position.column + inserted.len())
    } else {
        let last = inserted.rsplit('\n').next().unwrap_or("");
        Point::new(start_position.row + added_lines, last.len())
    };
    InputEdit {
        start_byte,
        old_end_byte: start_byte,
        new_end_byte: start_byte + inserted.len(),
        start_position,
        old_end_position: start_position,
        new_end_position,
    }
}

/// Builds the tree-sitter edit describing a deletion from `before`.
///
/// `before` is the buffer as it was prior to the deletion. The positions may be
/// supplied in either order.
#[must_use]
pub fn deletion_edit(before: &Buffer, start: Position, end: Position) -> InputEdit {
    let text = before.text();
    let a = locate(&text, start);
    let b = locate(&text, end);
    let ((start_byte, start_position), (old_end_byte, old_end_position)) =
        if a.0 <= b.0 { (a, b) } else { (b, a) };
    InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte: start_byte,
        start_position,
        old_end_position,
        new_end_position: start_position,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn kinds_on(hl: &Highlighter, buffer: &Buffer, line: usize) -> Vec<HighlightKind> {
        hl.highlight_line(buffer, line)
            .into_iter()
            .map(|s| s.kind)
            .collect()
    }

    fn highlighted(language: Language, text: &str) -> (Highlighter, Buffer) {
        let mut hl = Highlighter::new(language).expect("grammar loads");
        let buffer = Buffer::from_str(text);
        hl.parse(&buffer);
        (hl, buffer)
    }

    #[test]
    fn from_path_maps_known_extensions() {
        assert_eq!(Language::from_path(Path::new("a/b.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("m.PY")), Language::Python);
        assert_eq!(
            Language::from_path(Path::new("x.mjs")),
            Language::JavaScript
        );
        assert_eq!(
            Language::from_path(Path::new("notes.txt")),
            Language::PlainText
        );
        assert_eq!(
            Language::from_path(Path::new("Makefile")),
            Language::PlainText
        );
    }

    #[test]
    fn rust_line_has_a_keyword_and_a_string() {
        let (hl, buffer) = highlighted(Language::Rust, "fn main() {\n    let s = \"hi\";\n}\n");
        assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
        assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
    }

    #[test]
    fn python_line_has_a_keyword_and_a_string() {
        let (hl, buffer) = highlighted(Language::Python, "def f():\n    return \"hi\"\n");
        assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
        assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
    }

    #[test]
    fn javascript_line_has_a_keyword_and_a_string() {
        let (hl, buffer) = highlighted(
            Language::JavaScript,
            "function f() {\n  return \"hi\";\n}\n",
        );
        assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
        assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
    }

    #[test]
    fn plain_text_produces_no_spans() {
        let (hl, buffer) = highlighted(Language::PlainText, "fn main() {}\n");
        assert!(!hl.has_tree());
        assert!(hl.highlight_line(&buffer, 0).is_empty());
    }

    #[test]
    fn a_syntactically_broken_file_does_not_panic() {
        let (hl, buffer) = highlighted(Language::Rust, "fn ((( {{{ \"unterminated\nlet ] ) }\n");
        // Whatever the parser recovers, asking for spans must not panic and must
        // stay within each line.
        for line in 0..buffer.len_lines() {
            let len = buffer.line_len_chars(line);
            for span in hl.highlight_line(&buffer, line) {
                assert!(span.start_col < span.end_col);
                assert!(span.end_col <= len);
            }
        }
    }

    #[test]
    fn spans_use_character_columns_on_a_non_ascii_line() {
        let text = "let é = \"wörld\";\n";
        let (hl, buffer) = highlighted(Language::Rust, text);
        let spans = hl.highlight_line(&buffer, 0);
        let string_span = spans
            .iter()
            .find(|s| s.kind == HighlightKind::String)
            .expect("string span");
        let chars: Vec<char> = text.trim_end_matches('\n').chars().collect();
        assert_eq!(chars[string_span.start_col], '"');
        // A byte-offset bug would land one column late, on the 'w'.
        assert_eq!(string_span.start_col, 8);
        assert!(string_span.end_col <= chars.len());
    }

    #[test]
    fn out_of_range_lines_are_empty() {
        let (hl, buffer) = highlighted(Language::Rust, "fn main() {}\n");
        assert!(hl.highlight_line(&buffer, 999).is_empty());
    }

    #[test]
    fn cache_returns_the_same_spans_after_an_unrelated_edit() {
        let text = "fn a() {}\n\n\n\n\nfn b() {}\n";
        let mut hl = Highlighter::new(Language::Rust).expect("grammar loads");
        let mut buffer = Buffer::from_str(text);
        hl.parse(&buffer);
        let before = hl.highlight_line(&buffer, 0);
        assert_eq!(hl.cached_line_count(), 1);

        let at = Position::new(5, 9);
        let edit = insertion_edit(&buffer, at, " // tail");
        buffer.insert_str(at, " // tail");
        hl.edit(&edit);
        assert_eq!(hl.cached_line_count(), 0, "edit drops the cache");
        hl.parse(&buffer);

        let after = hl.highlight_line(&buffer, 0);
        assert_eq!(before, after);
        assert!(
            hl.highlight_line(&buffer, 5)
                .iter()
                .any(|s| s.kind == HighlightKind::Comment)
        );
    }

    #[test]
    fn deletion_edit_covers_the_removed_range_in_bytes() {
        let buffer = Buffer::from_str("aé\nbc\n");
        let edit = deletion_edit(&buffer, Position::new(0, 1), Position::new(1, 1));
        assert_eq!(edit.start_byte, 1);
        // 'é' is two bytes, then the newline, then one byte of line 1.
        assert_eq!(edit.old_end_byte, 5);
        assert_eq!(edit.new_end_byte, 1);
        assert_eq!(edit.start_position, Point::new(0, 1));
        assert_eq!(edit.old_end_position, Point::new(1, 1));
    }

    #[test]
    fn deletion_edit_accepts_reversed_positions() {
        let buffer = Buffer::from_str("abcdef\n");
        let forward = deletion_edit(&buffer, Position::new(0, 1), Position::new(0, 4));
        let reversed = deletion_edit(&buffer, Position::new(0, 4), Position::new(0, 1));
        assert_eq!(forward.start_byte, reversed.start_byte);
        assert_eq!(forward.old_end_byte, reversed.old_end_byte);
    }

    #[test]
    fn insertion_edit_tracks_added_lines() {
        let buffer = Buffer::from_str("abc\n");
        let edit = insertion_edit(&buffer, Position::new(0, 3), "\nxy");
        assert_eq!(edit.start_byte, 3);
        assert_eq!(edit.old_end_byte, 3);
        assert_eq!(edit.new_end_byte, 6);
        assert_eq!(edit.new_end_position, Point::new(1, 2));
    }

    #[test]
    fn unknown_capture_names_are_ignored() {
        assert_eq!(
            kind_for_capture("keyword.function"),
            Some(HighlightKind::Keyword)
        );
        assert_eq!(
            kind_for_capture("string.special"),
            Some(HighlightKind::String)
        );
        assert_eq!(kind_for_capture("embedded"), None);
        assert_eq!(kind_for_capture(""), None);
    }

    #[test]
    fn switching_language_drops_the_previous_tree() {
        let (mut hl, buffer) = highlighted(Language::Rust, "fn main() {}\n");
        assert!(hl.has_tree());
        hl.set_language(Language::PlainText).expect("plain text");
        assert!(!hl.has_tree());
        assert!(hl.highlight_line(&buffer, 0).is_empty());
    }

    #[test]
    fn byte_to_char_map_covers_multibyte_text() {
        let map = byte_to_char_map("aé b");
        assert_eq!(map[0], 0);
        assert_eq!(map[1], 1);
        assert_eq!(map[2], 1);
        assert_eq!(map[3], 2);
        assert_eq!(map[map.len() - 1], 4);
    }
}
