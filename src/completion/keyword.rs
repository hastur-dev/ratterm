//! Keyword-based completion provider.
//!
//! Provides completions based on:
//! 1. Language-specific keywords
//! 2. Words extracted from the current buffer

use std::collections::HashSet;

use super::provider::{
    CompletionContext, CompletionFuture, CompletionItem, CompletionKind, CompletionProvider,
    CompletionResult, MAX_COMPLETION_ITEMS,
};

/// Maximum words to extract from buffer.
const MAX_BUFFER_WORDS: usize = 5000;

/// Minimum word length to include.
const MIN_WORD_LENGTH: usize = 2;

/// Provider ID for keyword completions.
const PROVIDER_ID: &str = "keyword";

/// Priority for keyword provider (low, acts as fallback).
const PROVIDER_PRIORITY: u32 = 10;

/// Keyword completion provider.
#[derive(Debug, Default)]
pub struct KeywordProvider;

impl KeywordProvider {
    /// Creates a new keyword provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Extracts words from buffer content.
    fn extract_words(content: &str) -> HashSet<String> {
        let mut words = HashSet::with_capacity(MAX_BUFFER_WORDS);
        let mut word = String::new();
        let mut count = 0;

        for ch in content.chars() {
            if count >= MAX_BUFFER_WORDS {
                break;
            }

            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
            } else if !word.is_empty() {
                if word.len() >= MIN_WORD_LENGTH {
                    words.insert(std::mem::take(&mut word));
                    count += 1;
                } else {
                    word.clear();
                }
            }
        }

        // Don't forget the last word
        if !word.is_empty() && word.len() >= MIN_WORD_LENGTH && count < MAX_BUFFER_WORDS {
            words.insert(word);
        }

        words
    }

    /// Returns language keywords for the given language ID.
    fn language_keywords(language_id: &str) -> &'static [&'static str] {
        match language_id {
            "rust" => RUST_KEYWORDS,
            "python" => PYTHON_KEYWORDS,
            "javascript" | "typescript" | "javascriptreact" | "typescriptreact" => JS_KEYWORDS,
            "java" => JAVA_KEYWORDS,
            "csharp" | "cs" => CSHARP_KEYWORDS,
            "php" => PHP_KEYWORDS,
            "sql" | "postgres" | "postgresql" | "mysql" => SQL_KEYWORDS,
            "html" => HTML_KEYWORDS,
            "css" | "scss" | "less" => CSS_KEYWORDS,
            _ => &[],
        }
    }

    /// Filters and scores completions by prefix.
    fn filter_by_prefix(
        items: impl IntoIterator<Item = (String, CompletionKind)>,
        prefix: &str,
        word_at_cursor: &str,
    ) -> Vec<CompletionItem> {
        let prefix_lower = prefix.to_lowercase();
        let word_lower = word_at_cursor.to_lowercase();

        let mut results: Vec<CompletionItem> = items
            .into_iter()
            .filter(|(item, _)| {
                let item_lower = item.to_lowercase();
                // Match if item starts with prefix or word at cursor
                (item_lower.starts_with(&prefix_lower) || item_lower.starts_with(&word_lower))
                    // Don't suggest exact matches
                    && item_lower != word_lower
            })
            .map(|(label, kind)| {
                let priority = Self::score_match(&label, prefix, word_at_cursor);
                CompletionItem::new(label.clone(), label, kind, PROVIDER_ID.to_string())
                    .with_priority(priority)
            })
            .collect();

        // Sort by priority (descending) then by label
        results.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.label.cmp(&b.label))
        });

        // Limit results
        results.truncate(MAX_COMPLETION_ITEMS);
        results
    }

    /// Scores a match based on how well it matches the prefix.
    fn score_match(item: &str, prefix: &str, word_at_cursor: &str) -> u32 {
        let item_lower = item.to_lowercase();
        let prefix_lower = prefix.to_lowercase();
        let word_lower = word_at_cursor.to_lowercase();

        let mut score: u32 = 0;

        // Exact prefix match (highest priority)
        if item_lower.starts_with(&prefix_lower) && !prefix.is_empty() {
            score += 100;
        }

        // Word at cursor match
        if item_lower.starts_with(&word_lower) && !word_at_cursor.is_empty() {
            score += 50;
        }

        // Shorter items get slight boost
        score += (100_u32).saturating_sub(item.len() as u32);

        score
    }
}

impl CompletionProvider for KeywordProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn priority(&self) -> u32 {
        PROVIDER_PRIORITY
    }

    fn supports_language(&self, _language_id: &str) -> bool {
        // Keyword provider supports all languages
        true
    }

    fn complete(&self, context: &CompletionContext) -> CompletionFuture {
        let language_id = context.language_id.clone();
        let prefix = context.prefix.clone();
        let word_at_cursor = context.word_at_cursor.clone();
        let buffer_content = context.buffer_content.clone();

        Box::pin(async move {
            // Skip if no meaningful prefix
            if prefix.is_empty() && word_at_cursor.is_empty() {
                return None;
            }

            let mut items: Vec<(String, CompletionKind)> = Vec::new();

            // Add language keywords
            let keywords = Self::language_keywords(&language_id);
            for &kw in keywords {
                items.push((kw.to_string(), CompletionKind::Keyword));
            }

            // Add words from buffer
            if let Some(ref content) = buffer_content {
                let words = Self::extract_words(content);
                for word in words {
                    items.push((word, CompletionKind::Text));
                }
            }

            // Filter by prefix
            let filtered = Self::filter_by_prefix(items, &prefix, &word_at_cursor);

            if filtered.is_empty() {
                None
            } else {
                Some(CompletionResult::new(PROVIDER_ID, filtered))
            }
        })
    }
}

// ============================================================================
// Language Keywords
// ============================================================================

/// Rust keywords.
static RUST_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
    // Common types
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "f32",
    "f64",
    "bool",
    "char",
    "str",
    "String",
    "Vec",
    "Option",
    "Result",
    "Box",
    "Rc",
    "Arc",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "LinkedList",
    // Common macros
    "println",
    "print",
    "eprintln",
    "eprint",
    "format",
    "vec",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "todo",
    "unimplemented",
    "unreachable",
    "cfg",
    "include",
    "include_str",
    "include_bytes",
    "concat",
    "stringify",
    // Common traits
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Display",
    "Drop",
    "Eq",
    "From",
    "Into",
    "Iterator",
    "PartialEq",
    "PartialOrd",
    "Ord",
    "Send",
    "Sync",
    "Sized",
    "AsRef",
    "AsMut",
    "Deref",
];

/// Python keywords.
static PYTHON_KEYWORDS: &[&str] = &[
    "False",
    "None",
    "True",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "try",
    "while",
    "with",
    "yield",
    // Common builtins
    "abs",
    "all",
    "any",
    "bin",
    "bool",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
    // Common stdlib
    "self",
    "cls",
    "__init__",
    "__str__",
    "__repr__",
    "__len__",
    "__iter__",
    "__next__",
    "__enter__",
    "__exit__",
    "__call__",
    "__getitem__",
    "__setitem__",
    "__delitem__",
];

/// JavaScript/TypeScript keywords.
static JS_KEYWORDS: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "async",
    // TypeScript specific
    "type",
    "namespace",
    "declare",
    "readonly",
    "abstract",
    "as",
    "any",
    "boolean",
    "number",
    "string",
    "symbol",
    "never",
    "unknown",
    "object",
    "keyof",
    "infer",
    "is",
    "asserts",
    // Common globals
    "console",
    "window",
    "document",
    "Array",
    "Object",
    "String",
    "Number",
    "Boolean",
    "Date",
    "Math",
    "JSON",
    "Promise",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Symbol",
    "BigInt",
    "Error",
    "undefined",
    "NaN",
    "Infinity",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "fetch",
    "require",
    "module",
    "exports",
    "process",
    "Buffer",
];

/// Java keywords.
static JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    // Common types
    "String",
    "Integer",
    "Long",
    "Double",
    "Float",
    "Boolean",
    "Character",
    "Byte",
    "Short",
    "Object",
    "Class",
    "System",
    "List",
    "ArrayList",
    "LinkedList",
    "Map",
    "HashMap",
    "TreeMap",
    "Set",
    "HashSet",
    "TreeSet",
    "Collection",
    "Collections",
    "Arrays",
    "Optional",
    "Stream",
    "Collectors",
    "Comparator",
    "Iterator",
    "Iterable",
    "Exception",
    "RuntimeException",
    "IOException",
    "NullPointerException",
    "IllegalArgumentException",
    "Override",
    "Deprecated",
    "SuppressWarnings",
    "FunctionalInterface",
];

/// C# keywords.
static CSHARP_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "null",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
    "add",
    "alias",
    "ascending",
    "async",
    "await",
    "by",
    "descending",
    "dynamic",
    "equals",
    "from",
    "get",
    "global",
    "group",
    "into",
    "join",
    "let",
    "nameof",
    "on",
    "orderby",
    "partial",
    "remove",
    "select",
    "set",
    "value",
    "var",
    "when",
    "where",
    "yield",
    "record",
    "init",
    "required",
    "with",
    // Common types
    "String",
    "Int32",
    "Int64",
    "Double",
    "Boolean",
    "Object",
    "List",
    "Dictionary",
    "HashSet",
    "Queue",
    "Stack",
    "Array",
    "Console",
    "Task",
    "Func",
    "Action",
    "IEnumerable",
    "ICollection",
    "IList",
    "IDictionary",
    "Nullable",
    "Exception",
    "ArgumentException",
];

/// PHP keywords.
static PHP_KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "array",
    "as",
    "break",
    "callable",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "die",
    "do",
    "echo",
    "else",
    "elseif",
    "empty",
    "enddeclare",
    "endfor",
    "endforeach",
    "endif",
    "endswitch",
    "endwhile",
    "eval",
    "exit",
    "extends",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "include",
    "include_once",
    "instanceof",
    "insteadof",
    "interface",
    "isset",
    "list",
    "match",
    "namespace",
    "new",
    "or",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "static",
    "switch",
    "throw",
    "trait",
    "try",
    "unset",
    "use",
    "var",
    "while",
    "xor",
    "yield",
    "yield from",
    "true",
    "false",
    "null",
    "self",
    "parent",
    // Common functions
    "strlen",
    "strpos",
    "substr",
    "str_replace",
    "explode",
    "implode",
    "trim",
    "strtolower",
    "strtoupper",
    "array_push",
    "array_pop",
    "array_shift",
    "array_unshift",
    "array_merge",
    "array_map",
    "array_filter",
    "array_keys",
    "array_values",
    "count",
    "in_array",
    "json_encode",
    "json_decode",
    "file_get_contents",
    "file_put_contents",
    "preg_match",
    "preg_replace",
    "sprintf",
    "printf",
    "var_dump",
    "print_r",
];

/// SQL keywords.
static SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "IN",
    "BETWEEN",
    "LIKE",
    "IS",
    "NULL",
    "ORDER",
    "BY",
    "ASC",
    "DESC",
    "LIMIT",
    "OFFSET",
    "GROUP",
    "HAVING",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "CROSS",
    "ON",
    "AS",
    "DISTINCT",
    "ALL",
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "DATABASE",
    "SCHEMA",
    "INDEX",
    "VIEW",
    "DROP",
    "ALTER",
    "ADD",
    "COLUMN",
    "CONSTRAINT",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "UNIQUE",
    "CHECK",
    "DEFAULT",
    "NOT NULL",
    "AUTO_INCREMENT",
    "SERIAL",
    "IDENTITY",
    "CASCADE",
    "RESTRICT",
    "TRUNCATE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "TRANSACTION",
    "SAVEPOINT",
    "GRANT",
    "REVOKE",
    "TO",
    "WITH",
    "RECURSIVE",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "COALESCE",
    "NULLIF",
    "CAST",
    "CONVERT",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "EXISTS",
    "ANY",
    "SOME",
    "IF",
    "ELSIF",
    "LOOP",
    "WHILE",
    "FOR",
    "RETURN",
    "DECLARE",
    "CURSOR",
    "FETCH",
    "CLOSE",
    "OPEN",
    // PostgreSQL specific
    "RETURNING",
    "ILIKE",
    "SIMILAR",
    "ARRAY",
    "JSONB",
    "JSON",
    "TEXT",
    "VARCHAR",
    "INTEGER",
    "BIGINT",
    "SMALLINT",
    "BOOLEAN",
    "TIMESTAMP",
    "DATE",
    "TIME",
    "INTERVAL",
    "UUID",
    "BYTEA",
    "NUMERIC",
    "DECIMAL",
    "REAL",
    "DOUBLE PRECISION",
    "MONEY",
    "CIDR",
    "INET",
    "MACADDR",
];

/// HTML keywords (tags and attributes).
static HTML_KEYWORDS: &[&str] = &[
    "html",
    "head",
    "body",
    "title",
    "meta",
    "link",
    "script",
    "style",
    "div",
    "span",
    "p",
    "a",
    "img",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "td",
    "th",
    "thead",
    "tbody",
    "tfoot",
    "form",
    "input",
    "button",
    "select",
    "option",
    "textarea",
    "label",
    "fieldset",
    "legend",
    "header",
    "footer",
    "nav",
    "main",
    "section",
    "article",
    "aside",
    "figure",
    "figcaption",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "mark",
    "small",
    "sub",
    "sup",
    "br",
    "hr",
    "pre",
    "code",
    "blockquote",
    "cite",
    "abbr",
    "address",
    "time",
    "progress",
    "meter",
    "details",
    "summary",
    "dialog",
    "canvas",
    "svg",
    "video",
    "audio",
    "source",
    "track",
    "iframe",
    "embed",
    "object",
    "param",
    "picture",
    "template",
    "slot",
    "noscript",
    // Common attributes
    "class",
    "id",
    "style",
    "src",
    "href",
    "alt",
    "title",
    "type",
    "name",
    "value",
    "placeholder",
    "disabled",
    "readonly",
    "required",
    "checked",
    "selected",
    "hidden",
    "target",
    "rel",
    "data",
    "aria",
    "role",
    "tabindex",
    "lang",
    "dir",
    "contenteditable",
    "draggable",
    "spellcheck",
    "autocomplete",
    "autofocus",
    "pattern",
    "min",
    "max",
    "step",
    "width",
    "height",
    "colspan",
    "rowspan",
    "action",
    "method",
    "enctype",
    "accept",
];

/// CSS keywords (properties and values).
static CSS_KEYWORDS: &[&str] = &[
    // Properties
    "display",
    "position",
    "top",
    "right",
    "bottom",
    "left",
    "float",
    "clear",
    "z-index",
    "overflow",
    "visibility",
    "opacity",
    "width",
    "height",
    "min-width",
    "max-width",
    "min-height",
    "max-height",
    "margin",
    "padding",
    "border",
    "border-radius",
    "outline",
    "background",
    "background-color",
    "background-image",
    "background-position",
    "background-size",
    "background-repeat",
    "color",
    "font",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration",
    "text-transform",
    "text-indent",
    "text-shadow",
    "white-space",
    "word-wrap",
    "word-break",
    "vertical-align",
    "box-shadow",
    "box-sizing",
    "cursor",
    "transform",
    "transition",
    "animation",
    "flex",
    "flex-direction",
    "flex-wrap",
    "justify-content",
    "align-items",
    "align-content",
    "align-self",
    "order",
    "flex-grow",
    "flex-shrink",
    "flex-basis",
    "grid",
    "grid-template-columns",
    "grid-template-rows",
    "grid-column",
    "grid-row",
    "gap",
    "column-gap",
    "row-gap",
    "place-items",
    "place-content",
    // Values
    "none",
    "block",
    "inline",
    "inline-block",
    "flex",
    "grid",
    "absolute",
    "relative",
    "fixed",
    "sticky",
    "static",
    "auto",
    "hidden",
    "visible",
    "scroll",
    "inherit",
    "initial",
    "unset",
    "transparent",
    "solid",
    "dashed",
    "dotted",
    "double",
    "groove",
    "ridge",
    "inset",
    "outset",
    "normal",
    "bold",
    "italic",
    "underline",
    "uppercase",
    "lowercase",
    "capitalize",
    "nowrap",
    "center",
    "left",
    "right",
    "top",
    "bottom",
    "middle",
    "baseline",
    "pointer",
    "default",
    "row",
    "column",
    "wrap",
    "space-between",
    "space-around",
    "space-evenly",
    "stretch",
    "start",
    "end",
    "flex-start",
    "flex-end",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "linear",
    "infinite",
    "alternate",
    "forwards",
    "backwards",
    "both",
    "running",
    "paused",
    // Units
    "px",
    "em",
    "rem",
    "vh",
    "vw",
    "vmin",
    "vmax",
    "ch",
    "ex",
    "deg",
    "rad",
    "turn",
    "ms",
    "s",
    // Functions
    "var",
    "calc",
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "url",
    "linear-gradient",
    "radial-gradient",
    "translate",
    "rotate",
    "scale",
    "skew",
    "matrix",
    "perspective",
];

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_words() {
        let content = "let foo = bar + baz; // comment";
        let words = KeywordProvider::extract_words(content);

        assert!(words.contains("let"));
        assert!(words.contains("foo"));
        assert!(words.contains("bar"));
        assert!(words.contains("baz"));
        assert!(words.contains("comment"));
    }

    #[test]
    fn test_extract_words_min_length() {
        let content = "a b c de fg hij";
        let words = KeywordProvider::extract_words(content);

        assert!(!words.contains("a"));
        assert!(!words.contains("b"));
        assert!(!words.contains("c"));
        assert!(words.contains("de"));
        assert!(words.contains("fg"));
        assert!(words.contains("hij"));
    }

    #[test]
    fn test_language_keywords_rust() {
        let kw = KeywordProvider::language_keywords("rust");
        assert!(!kw.is_empty());
        assert!(kw.contains(&"fn"));
        assert!(kw.contains(&"let"));
        assert!(kw.contains(&"impl"));
    }

    #[test]
    fn test_language_keywords_python() {
        let kw = KeywordProvider::language_keywords("python");
        assert!(!kw.is_empty());
        assert!(kw.contains(&"def"));
        assert!(kw.contains(&"class"));
        assert!(kw.contains(&"import"));
    }

    #[test]
    fn test_language_keywords_unknown() {
        let kw = KeywordProvider::language_keywords("unknown_lang");
        assert!(kw.is_empty());
    }

    #[test]
    fn test_filter_by_prefix() {
        let items = vec![
            ("foo".to_string(), CompletionKind::Variable),
            ("foobar".to_string(), CompletionKind::Variable),
            ("bar".to_string(), CompletionKind::Variable),
            ("baz".to_string(), CompletionKind::Variable),
        ];

        let filtered = KeywordProvider::filter_by_prefix(items, "fo", "fo");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|i| i.label == "foo"));
        assert!(filtered.iter().any(|i| i.label == "foobar"));
    }

    #[test]
    fn test_filter_excludes_exact_match() {
        let items = vec![
            ("foo".to_string(), CompletionKind::Variable),
            ("foobar".to_string(), CompletionKind::Variable),
        ];

        let filtered = KeywordProvider::filter_by_prefix(items, "foo", "foo");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "foobar");
    }

    #[tokio::test]
    async fn test_keyword_provider_complete() {
        let provider = KeywordProvider::new();

        // Use "le" prefix which should match "let" keyword
        let context = CompletionContext::new("rust", 0, 2)
            .with_prefix("le")
            .with_word_at_cursor("le")
            .with_buffer_content("fn main() { let x = 1; }");

        let result = provider.complete(&context).await;
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(!result.is_empty());
        assert_eq!(result.provider_id, "keyword");
        // Should find "let" keyword
        assert!(result.items.iter().any(|i| i.label == "let"));
    }

    #[test]
    fn test_provider_supports_all_languages() {
        let provider = KeywordProvider::new();
        assert!(provider.supports_language("rust"));
        assert!(provider.supports_language("python"));
        assert!(provider.supports_language("unknown"));
    }

    #[test]
    fn test_provider_id_and_priority() {
        let provider = KeywordProvider::new();
        assert_eq!(provider.id(), "keyword");
        assert_eq!(provider.priority(), 10);
    }
}
