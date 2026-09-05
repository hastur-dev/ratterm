//! Languages the editor understands, and the renderer-agnostic highlight
//! categories their grammars map onto.
//!
//! Split out of `highlight.rs` so neither file grows past the project's size
//! limit. [`highlight`](super::highlight) re-exports everything here, so
//! `crate::editor::highlight::Language` keeps working.

use std::path::Path;

use tree_sitter::Language as TsLanguage;

/// A language the editor can highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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

impl HighlightSpan {
    /// Returns true when `col` falls inside the run.
    #[must_use]
    pub const fn covers(&self, col: usize) -> bool {
        col >= self.start_col && col < self.end_col
    }
}

/// Returns the kind covering `col`, if any span does.
#[must_use]
pub fn kind_at(spans: &[HighlightSpan], col: usize) -> Option<HighlightKind> {
    spans.iter().find(|s| s.covers(col)).map(|s| s.kind)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

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
    fn every_language_reports_consistent_traits() {
        assert!(Language::Rust.uses_braces());
        assert!(!Language::Python.uses_braces());
        assert!(Language::Python.single_quote_is_string());
        assert!(!Language::Rust.single_quote_is_string());
        assert_eq!(Language::Rust.line_comment(), Some("//"));
        assert_eq!(Language::Python.line_comment(), Some("#"));
        assert_eq!(Language::PlainText.line_comment(), None);
        assert_eq!(Language::PlainText.name(), "Plain Text");
    }

    #[test]
    fn only_plain_text_lacks_a_grammar() {
        for language in [Language::Rust, Language::Python, Language::JavaScript] {
            assert!(language.ts_language().is_some(), "{language:?}");
            assert!(language.highlights_query().is_some(), "{language:?}");
        }
        assert!(Language::PlainText.ts_language().is_none());
        assert!(Language::PlainText.highlights_query().is_none());
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
    fn kind_at_finds_the_covering_span() {
        let spans = vec![
            HighlightSpan {
                start_col: 0,
                end_col: 2,
                kind: HighlightKind::Keyword,
            },
            HighlightSpan {
                start_col: 5,
                end_col: 9,
                kind: HighlightKind::String,
            },
        ];
        assert_eq!(kind_at(&spans, 0), Some(HighlightKind::Keyword));
        assert_eq!(kind_at(&spans, 1), Some(HighlightKind::Keyword));
        assert_eq!(kind_at(&spans, 2), None);
        assert_eq!(kind_at(&spans, 8), Some(HighlightKind::String));
        assert_eq!(kind_at(&spans, 9), None);
        assert_eq!(kind_at(&[], 0), None);
    }
}
