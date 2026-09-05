//! Tree-sitter syntax highlighting.
//!
//! The grammars shipped with the crate (Rust, Python, JavaScript) each expose a
//! highlight query, so highlighting is driven by those queries rather than by a
//! hand-written node-kind table. Capture names are mapped to [`HighlightKind`],
//! a renderer-agnostic set; the theme layer decides what colour each kind gets.
//!
//! Every column reported here is a character offset within a line, never a byte
//! offset. Tree-sitter reports byte offsets, so [`Highlighter`] converts them
//! against the line text before returning spans.
//!
//! Parsing reads the rope in chunks rather than copying the document into a
//! `String`. That is what keeps a keystroke in a large file cheap: with a
//! previous tree in hand, tree-sitter only reads around the edit.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use thiserror::Error;
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

use super::buffer::Buffer;

pub use super::highlight_edit::{deletion_edit, insertion_edit, replacement_edit};
pub use super::language::{HighlightKind, HighlightSpan, Language, kind_at, kind_for_capture};

/// Files at or above this size are not parsed.
///
/// A full reparse of a very large file on every edit costs more than the
/// highlighting is worth, and the editor already opens files this size
/// read-only.
pub const MAX_HIGHLIGHT_BYTES: usize = 2 * 1024 * 1024;

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
/// change so tree-sitter can reuse the previous tree.
/// [`Highlighter::highlight_line`] is cheap to call repeatedly: results are
/// cached per line and the cache is dropped whenever the tree changes.
pub struct Highlighter {
    language: Language,
    parser: Option<Parser>,
    query: Option<Query>,
    tree: Option<Tree>,
    cache: RefCell<HashMap<usize, Vec<HighlightSpan>>>,
}

impl fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Highlighter")
            .field("language", &self.language)
            .field("has_tree", &self.tree.is_some())
            .finish()
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::plain()
    }
}

impl Highlighter {
    /// Creates a highlighter that produces no spans.
    ///
    /// Unlike [`Highlighter::new`] this cannot fail, so it is what an editor
    /// with no file open starts from.
    #[must_use]
    pub fn plain() -> Self {
        Self {
            language: Language::PlainText,
            parser: None,
            query: None,
            tree: None,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Creates a highlighter for `language`.
    ///
    /// # Errors
    /// Returns an error if the grammar or its highlight query cannot be loaded.
    pub fn new(language: Language) -> Result<Self, HighlightError> {
        let mut this = Self::plain();
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

    /// Drops the tree so the next parse starts from scratch.
    ///
    /// Used when a change reached the buffer without a matching
    /// [`Highlighter::edit`], where reusing the tree would misplace every span.
    pub fn invalidate(&mut self) {
        self.tree = None;
        self.cache.borrow_mut().clear();
    }

    /// Parses `buffer`, reusing the previous tree when one exists.
    ///
    /// Failure is silent by design: a document that cannot be parsed simply has
    /// no spans, so the renderer never has to handle an error.
    pub fn parse(&mut self, buffer: &Buffer) {
        self.cache.borrow_mut().clear();

        let Some(parser) = self.parser.as_mut() else {
            self.tree = None;
            return;
        };

        if buffer.len_bytes() >= MAX_HIGHLIGHT_BYTES {
            self.tree = None;
            return;
        }

        // Reading through the rope means an incremental reparse touches only
        // the chunks around the edit instead of copying the whole document.
        let mut read = |byte: usize, _: Point| buffer.chunk_at_byte(byte);
        self.tree = parser.parse_with_options(&mut read, self.tree.as_ref(), None);
        if self.tree.is_none() {
            tracing::debug!("tree-sitter returned no tree for {}", self.language.name());
        }
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
        if line >= buffer.len_lines() {
            return Vec::new();
        }
        let text = buffer.line_text(line);
        let byte_len = text.len();
        let byte_to_char = byte_to_char_map(&text);
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
        let mut text_of = |node: Node| std::iter::once(buffer.byte_range(node.byte_range()));
        let mut matches = cursor.matches(query, tree.root_node(), &mut text_of);
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

#[cfg(test)]
mod tests;
