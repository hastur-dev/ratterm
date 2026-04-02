//! Document and workspace symbols.

use std::path::PathBuf;
use serde_json::Value as JsonValue;
use super::definition::uri_to_path;
use super::hover::TextRange;

/// Symbol kind (matches LSP SymbolKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Unknown,
}

impl SymbolKind {
    pub fn from_lsp(kind: u64) -> Self {
        match kind {
            1 => Self::File,
            2 => Self::Module,
            3 => Self::Namespace,
            4 => Self::Package,
            5 => Self::Class,
            6 => Self::Method,
            7 => Self::Property,
            8 => Self::Field,
            9 => Self::Constructor,
            10 => Self::Enum,
            11 => Self::Interface,
            12 => Self::Function,
            13 => Self::Variable,
            14 => Self::Constant,
            15 => Self::String,
            16 => Self::Number,
            17 => Self::Boolean,
            18 => Self::Array,
            19 => Self::Object,
            20 => Self::Key,
            21 => Self::Null,
            22 => Self::EnumMember,
            23 => Self::Struct,
            24 => Self::Event,
            25 => Self::Operator,
            26 => Self::TypeParameter,
            _ => Self::Unknown,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Function | Self::Method => "fn",
            Self::Class | Self::Struct => "S",
            Self::Enum => "E",
            Self::Interface => "I",
            Self::Variable => "v",
            Self::Constant => "C",
            Self::Field | Self::Property => "f",
            Self::Module | Self::Namespace => "M",
            _ => " ",
        }
    }
}

/// A document symbol (hierarchical).
#[derive(Debug, Clone)]
pub struct DocumentSymbolResult {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub children: Vec<DocumentSymbolResult>,
}

/// A workspace symbol (flat).
#[derive(Debug, Clone)]
pub struct SymbolInfoResult {
    pub name: String,
    pub kind: SymbolKind,
    pub path: PathBuf,
    pub line: u32,
    pub character: u32,
    pub container_name: Option<String>,
}

/// Parses document symbols response.
pub fn parse_document_symbols(result: JsonValue) -> Vec<DocumentSymbolResult> {
    if result.is_null() {
        return Vec::new();
    }

    let Some(arr) = result.as_array() else {
        return Vec::new();
    };

    // Check if it's DocumentSymbol[] or SymbolInformation[]
    if arr.first().and_then(|f| f.get("range")).is_some() {
        // DocumentSymbol format (hierarchical)
        arr.iter().filter_map(parse_document_symbol).collect()
    } else {
        // SymbolInformation format (flat) - convert to DocumentSymbol
        arr.iter()
            .filter_map(|item| {
                let name = item.get("name")?.as_str()?.to_string();
                let kind = item
                    .get("kind")
                    .and_then(|k| k.as_u64())
                    .map(SymbolKind::from_lsp)
                    .unwrap_or(SymbolKind::Unknown);
                let location = item.get("location")?;
                let range_val = location.get("range")?;
                let range = super::hover::parse_range(range_val)?;
                Some(DocumentSymbolResult {
                    name,
                    detail: None,
                    kind,
                    range,
                    selection_range: range,
                    children: Vec::new(),
                })
            })
            .collect()
    }
}

fn parse_document_symbol(val: &JsonValue) -> Option<DocumentSymbolResult> {
    let name = val.get("name")?.as_str()?.to_string();
    let detail = val
        .get("detail")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());
    let kind = val
        .get("kind")
        .and_then(|k| k.as_u64())
        .map(SymbolKind::from_lsp)
        .unwrap_or(SymbolKind::Unknown);
    let range = super::hover::parse_range(val.get("range")?)?;
    let selection_range = val
        .get("selectionRange")
        .and_then(super::hover::parse_range)
        .unwrap_or(range);

    let children = val
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(parse_document_symbol).collect())
        .unwrap_or_default();

    Some(DocumentSymbolResult {
        name,
        detail,
        kind,
        range,
        selection_range,
        children,
    })
}

/// Parses workspace symbols response.
pub fn parse_workspace_symbols(result: JsonValue) -> Vec<SymbolInfoResult> {
    if result.is_null() {
        return Vec::new();
    }

    let Some(arr) = result.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let kind = item
                .get("kind")
                .and_then(|k| k.as_u64())
                .map(SymbolKind::from_lsp)
                .unwrap_or(SymbolKind::Unknown);
            let container_name = item
                .get("containerName")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            let location = item.get("location")?;
            let uri = location.get("uri")?.as_str()?;
            let path = uri_to_path(uri)?;
            let range = location.get("range")?;
            let start = range.get("start")?;
            let line = start.get("line")?.as_u64()? as u32;
            let character = start.get("character")?.as_u64()? as u32;

            Some(SymbolInfoResult {
                name,
                kind,
                path,
                line,
                character,
                container_name,
            })
        })
        .collect()
}

/// Flattens hierarchical symbols into a list with depth info.
pub fn flatten_symbols(
    symbols: &[DocumentSymbolResult],
    depth: usize,
) -> Vec<(usize, &DocumentSymbolResult)> {
    let mut result = Vec::new();
    for symbol in symbols {
        result.push((depth, symbol));
        result.extend(flatten_symbols(&symbol.children, depth + 1));
    }
    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_document_symbols_hierarchical() {
        let resp = json!([
            {
                "name": "main",
                "kind": 12,
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
                "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 7}},
                "children": [
                    {
                        "name": "x",
                        "kind": 13,
                        "range": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 10}},
                        "selectionRange": {"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 9}}
                    }
                ]
            }
        ]);
        let symbols = parse_document_symbols(resp);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[0].children.len(), 1);
    }

    #[test]
    fn test_parse_workspace_symbols() {
        let resp = json!([
            {
                "name": "MyStruct",
                "kind": 23,
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": {"start": {"line": 10, "character": 0}, "end": {"line": 20, "character": 1}}
                },
                "containerName": "mymod"
            }
        ]);
        let symbols = parse_workspace_symbols(resp);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_flatten_symbols() {
        let symbols = vec![DocumentSymbolResult {
            name: "A".into(),
            detail: None,
            kind: SymbolKind::Function,
            range: TextRange {
                start_line: 0,
                start_char: 0,
                end_line: 5,
                end_char: 0,
            },
            selection_range: TextRange {
                start_line: 0,
                start_char: 0,
                end_line: 0,
                end_char: 1,
            },
            children: vec![DocumentSymbolResult {
                name: "B".into(),
                detail: None,
                kind: SymbolKind::Variable,
                range: TextRange {
                    start_line: 1,
                    start_char: 0,
                    end_line: 1,
                    end_char: 5,
                },
                selection_range: TextRange {
                    start_line: 1,
                    start_char: 0,
                    end_line: 1,
                    end_char: 1,
                },
                children: Vec::new(),
            }],
        }];
        let flat = flatten_symbols(&symbols, 0);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].0, 0); // depth 0
        assert_eq!(flat[1].0, 1); // depth 1
    }

    #[test]
    fn test_symbol_kind_from_lsp() {
        assert_eq!(SymbolKind::from_lsp(12), SymbolKind::Function);
        assert_eq!(SymbolKind::from_lsp(23), SymbolKind::Struct);
        assert_eq!(SymbolKind::from_lsp(999), SymbolKind::Unknown);
    }

    #[test]
    fn test_symbol_kind_icon() {
        assert_eq!(SymbolKind::Function.icon(), "fn");
        assert_eq!(SymbolKind::Struct.icon(), "S");
        assert_eq!(SymbolKind::Enum.icon(), "E");
        assert_eq!(SymbolKind::Variable.icon(), "v");
    }

    #[test]
    fn test_parse_document_symbols_null() {
        assert!(parse_document_symbols(JsonValue::Null).is_empty());
    }

    #[test]
    fn test_parse_workspace_symbols_null() {
        assert!(parse_workspace_symbols(JsonValue::Null).is_empty());
    }

    #[test]
    fn test_parse_symbol_information_format() {
        // SymbolInformation format (flat, with location instead of range)
        let resp = json!([
            {
                "name": "foo",
                "kind": 12,
                "location": {
                    "uri": "file:///src/main.rs",
                    "range": {"start": {"line": 5, "character": 0}, "end": {"line": 10, "character": 1}}
                }
            }
        ]);
        // This should NOT match the hierarchical path since there's no direct "range" field
        let symbols = parse_document_symbols(resp);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "foo");
    }
}
