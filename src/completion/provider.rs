//! Completion provider types and trait definitions.
//!
//! Defines the interface for completion providers (LSP, keywords, extensions)
//! and the data types used for completion requests and responses.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

/// Maximum number of completion items to return from any provider.
pub const MAX_COMPLETION_ITEMS: usize = 100;

/// Completion item kind (matches LSP CompletionItemKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompletionKind {
    #[default]
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
    Folder,
    EnumMember,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

impl CompletionKind {
    /// Returns a short display string for the kind.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Method => "method",
            Self::Function => "fn",
            Self::Constructor => "ctor",
            Self::Field => "field",
            Self::Variable => "var",
            Self::Class => "class",
            Self::Interface => "iface",
            Self::Module => "mod",
            Self::Property => "prop",
            Self::Unit => "unit",
            Self::Value => "val",
            Self::Enum => "enum",
            Self::Keyword => "kw",
            Self::Snippet => "snip",
            Self::Color => "color",
            Self::File => "file",
            Self::Reference => "ref",
            Self::Folder => "dir",
            Self::EnumMember => "member",
            Self::Constant => "const",
            Self::Struct => "struct",
            Self::Event => "event",
            Self::Operator => "op",
            Self::TypeParameter => "type",
        }
    }
}

/// A single completion suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// The text to insert when accepting this completion.
    pub insert_text: String,

    /// Display label shown in the completion list.
    pub label: String,

    /// Kind of completion item (function, variable, keyword, etc.).
    pub kind: CompletionKind,

    /// Optional detail or documentation string.
    pub detail: Option<String>,

    /// Priority for sorting (higher values appear first).
    /// Used to merge results from multiple providers.
    pub priority: u32,

    /// Identifier of the provider that generated this item.
    pub source: String,

    /// Filter text used for fuzzy matching (defaults to label if not set).
    pub filter_text: Option<String>,

    /// Sort text used for ordering (defaults to label if not set).
    pub sort_text: Option<String>,
}

impl CompletionItem {
    /// Creates a new completion item with required fields.
    #[must_use]
    pub fn new(insert_text: String, label: String, kind: CompletionKind, source: String) -> Self {
        assert!(!insert_text.is_empty(), "insert_text must not be empty");
        assert!(!label.is_empty(), "label must not be empty");

        Self {
            insert_text,
            label,
            kind,
            detail: None,
            priority: 0,
            source,
            filter_text: None,
            sort_text: None,
        }
    }

    /// Sets the detail string.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets the priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the filter text.
    #[must_use]
    pub fn with_filter_text(mut self, filter_text: impl Into<String>) -> Self {
        self.filter_text = Some(filter_text.into());
        self
    }

    /// Returns the text to use for filtering.
    #[must_use]
    pub fn filter_text_or_label(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }

    /// Returns the text to use for sorting.
    #[must_use]
    pub fn sort_text_or_label(&self) -> &str {
        self.sort_text.as_deref().unwrap_or(&self.label)
    }
}

/// Context for a completion request.
#[derive(Debug, Clone, Default)]
pub struct CompletionContext {
    /// Full file path (if known).
    pub file_path: Option<PathBuf>,

    /// Language identifier (e.g., "rust", "python", "javascript").
    pub language_id: String,

    /// Full content of the current line.
    pub line_content: String,

    /// Cursor line (0-indexed).
    pub line: usize,

    /// Cursor column (0-indexed, character offset).
    pub col: usize,

    /// Text before the cursor on the current line.
    pub prefix: String,

    /// The word at the cursor position (for filtering).
    pub word_at_cursor: String,

    /// Trigger character if completion was triggered by a specific character.
    pub trigger_char: Option<char>,

    /// Full buffer content (for keyword extraction).
    pub buffer_content: Option<String>,
}

impl CompletionContext {
    /// Creates a new completion context.
    #[must_use]
    pub fn new(language_id: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            language_id: language_id.into(),
            line,
            col,
            ..Default::default()
        }
    }

    /// Sets the file path.
    #[must_use]
    pub fn with_file_path(mut self, path: PathBuf) -> Self {
        self.file_path = Some(path);
        self
    }

    /// Sets the line content.
    #[must_use]
    pub fn with_line_content(mut self, content: impl Into<String>) -> Self {
        self.line_content = content.into();
        self
    }

    /// Sets the prefix (text before cursor).
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Sets the word at cursor.
    #[must_use]
    pub fn with_word_at_cursor(mut self, word: impl Into<String>) -> Self {
        self.word_at_cursor = word.into();
        self
    }

    /// Sets the trigger character.
    #[must_use]
    pub const fn with_trigger_char(mut self, ch: char) -> Self {
        self.trigger_char = Some(ch);
        self
    }

    /// Sets the full buffer content.
    #[must_use]
    pub fn with_buffer_content(mut self, content: impl Into<String>) -> Self {
        self.buffer_content = Some(content.into());
        self
    }

    /// Returns whether this context has a valid position.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.language_id.is_empty()
    }
}

/// Result from a completion provider.
#[derive(Debug, Clone)]
pub struct CompletionResult {
    /// Provider identifier.
    pub provider_id: String,

    /// List of completion items.
    pub items: Vec<CompletionItem>,

    /// Whether the result is complete (false means more items may be available).
    pub is_complete: bool,
}

impl CompletionResult {
    /// Creates a new completion result.
    #[must_use]
    pub fn new(provider_id: impl Into<String>, items: Vec<CompletionItem>) -> Self {
        let provider_id = provider_id.into();
        assert!(!provider_id.is_empty(), "provider_id must not be empty");

        Self {
            provider_id,
            items,
            is_complete: true,
        }
    }

    /// Marks the result as incomplete (more items available).
    #[must_use]
    pub const fn incomplete(mut self) -> Self {
        self.is_complete = false;
        self
    }

    /// Returns whether this result has any items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Type alias for boxed async completion future.
pub type CompletionFuture =
    Pin<Box<dyn Future<Output = Option<CompletionResult>> + Send + 'static>>;

/// Trait for completion providers.
///
/// Implementations can provide completions from various sources:
/// - LSP language servers
/// - Keyword/word extraction from buffer
/// - External AI services
/// - Custom extension providers
pub trait CompletionProvider: Send + Sync {
    /// Returns the unique identifier for this provider.
    fn id(&self) -> &str;

    /// Returns the priority of this provider (higher = earlier in results).
    ///
    /// Default priorities:
    /// - LSP: 100
    /// - Extensions: 50
    /// - Keywords: 10
    fn priority(&self) -> u32;

    /// Returns whether this provider supports the given language.
    fn supports_language(&self, language_id: &str) -> bool;

    /// Computes completions for the given context.
    ///
    /// Returns `None` if the provider declines to provide completions
    /// (e.g., not connected, language not supported, etc.).
    fn complete(&self, context: &CompletionContext) -> CompletionFuture;

    /// Called when the provider should shut down.
    ///
    /// Default implementation does nothing.
    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(async {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_item_creation() {
        let item = CompletionItem::new(
            "println!".to_string(),
            "println!".to_string(),
            CompletionKind::Function,
            "test".to_string(),
        );

        assert_eq!(item.insert_text, "println!");
        assert_eq!(item.label, "println!");
        assert_eq!(item.kind, CompletionKind::Function);
        assert_eq!(item.source, "test");
        assert!(item.detail.is_none());
        assert_eq!(item.priority, 0);
    }

    #[test]
    fn test_completion_item_builder() {
        let item = CompletionItem::new(
            "test".to_string(),
            "test".to_string(),
            CompletionKind::Variable,
            "kw".to_string(),
        )
        .with_detail("A test variable")
        .with_priority(50);

        assert_eq!(item.detail.as_deref(), Some("A test variable"));
        assert_eq!(item.priority, 50);
    }

    #[test]
    fn test_completion_context_creation() {
        let ctx = CompletionContext::new("rust", 10, 5)
            .with_line_content("let x = ")
            .with_prefix("let x = ")
            .with_word_at_cursor("");

        assert_eq!(ctx.language_id, "rust");
        assert_eq!(ctx.line, 10);
        assert_eq!(ctx.col, 5);
        assert!(ctx.is_valid());
    }

    #[test]
    fn test_completion_result() {
        let items = vec![
            CompletionItem::new(
                "foo".to_string(),
                "foo".to_string(),
                CompletionKind::Variable,
                "test".to_string(),
            ),
            CompletionItem::new(
                "bar".to_string(),
                "bar".to_string(),
                CompletionKind::Variable,
                "test".to_string(),
            ),
        ];

        let result = CompletionResult::new("test", items);
        assert_eq!(result.provider_id, "test");
        assert_eq!(result.len(), 2);
        assert!(!result.is_empty());
        assert!(result.is_complete);
    }

    #[test]
    fn test_completion_kind_display() {
        assert_eq!(CompletionKind::Function.as_str(), "fn");
        assert_eq!(CompletionKind::Keyword.as_str(), "kw");
        assert_eq!(CompletionKind::Variable.as_str(), "var");
    }
}
