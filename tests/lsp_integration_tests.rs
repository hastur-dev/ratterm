//! Integration tests for LSP features.
//!
//! Tests LSP response parsing, state management, and feature integration
//! using mock LSP JSON responses.

#[allow(clippy::expect_used, clippy::unwrap_used)]

mod lsp_hover_tests {
    use ratterm::lsp::hover::{parse_hover_response, hover_to_styled_lines, HoverContent};
    use serde_json::json;

    #[test]
    fn test_hover_popup_appears_with_valid_response() {
        let resp = json!({
            "contents": {
                "kind": "markdown",
                "value": "```rust\nfn main() -> ()\n```\nThe main entry point."
            },
            "range": {
                "start": {"line": 5, "character": 3},
                "end": {"line": 5, "character": 7}
            }
        });

        let hover = parse_hover_response(resp).expect("Should parse hover");
        assert!(!hover.contents.is_empty());
        assert!(hover.range.is_some());

        let lines = hover_to_styled_lines(&hover);
        assert!(!lines.is_empty());
        // Should have code lines and text lines
        let has_code = lines.iter().any(|(_, is_code)| *is_code);
        let has_text = lines.iter().any(|(_, is_code)| !*is_code);
        assert!(has_code, "Hover should contain code lines");
        assert!(has_text, "Hover should contain text lines");
    }

    #[test]
    fn test_hover_disappears_with_null_response() {
        let resp = json!({});
        assert!(parse_hover_response(resp).is_none());
    }

    #[test]
    fn test_hover_marked_string_array() {
        let resp = json!({
            "contents": [
                {"language": "rust", "value": "pub fn foo()"},
                "A function that does things."
            ]
        });
        let hover = parse_hover_response(resp).unwrap();
        assert_eq!(hover.contents.len(), 2);
        assert!(matches!(&hover.contents[0], HoverContent::Code { .. }));
        assert!(matches!(&hover.contents[1], HoverContent::Text(_)));
    }
}

mod lsp_definition_tests {
    use ratterm::lsp::definition::parse_location_response;
    use serde_json::json;

    #[test]
    fn test_goto_def_opens_correct_file_and_line() {
        let resp = json!({
            "uri": "file:///home/user/src/lib.rs",
            "range": {
                "start": {"line": 42, "character": 4},
                "end": {"line": 42, "character": 20}
            }
        });
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 42);
        assert_eq!(locations[0].character, 4);
    }

    #[test]
    fn test_goto_def_handles_location_link() {
        let resp = json!([{
            "originSelectionRange": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}},
            "targetUri": "file:///src/target.rs",
            "targetRange": {"start": {"line": 10, "character": 0}, "end": {"line": 20, "character": 0}},
            "targetSelectionRange": {"start": {"line": 10, "character": 4}, "end": {"line": 10, "character": 12}}
        }]);
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 10);
        assert_eq!(locations[0].character, 4);
    }

    #[test]
    fn test_goto_def_multiple_locations() {
        let resp = json!([
            {"uri": "file:///a.rs", "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}},
            {"uri": "file:///b.rs", "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 5}}}
        ]);
        let locations = parse_location_response(resp);
        assert_eq!(locations.len(), 2);
    }

    #[test]
    fn test_uri_path_roundtrip() {
        #[cfg(not(windows))]
        {
            use ratterm::lsp::definition::{path_to_uri, uri_to_path};
            let path = std::path::Path::new("/home/user/src/main.rs");
            let uri = path_to_uri(path);
            let back = uri_to_path(&uri).unwrap();
            assert_eq!(back, std::path::PathBuf::from("/home/user/src/main.rs"));
        }
    }
}

mod lsp_references_tests {
    use ratterm::lsp::references::parse_references_response;
    use serde_json::json;

    #[test]
    fn test_references_grouped_by_file() {
        let resp = json!([
            {"uri": "file:///src/main.rs", "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}},
            {"uri": "file:///src/main.rs", "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 5}}},
            {"uri": "file:///src/lib.rs", "range": {"start": {"line": 3, "character": 2}, "end": {"line": 3, "character": 8}}},
            {"uri": "file:///src/lib.rs", "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 6}}}
        ]);
        let groups = parse_references_response(resp);
        assert_eq!(groups.len(), 2, "Should have 2 file groups");

        let total_refs: usize = groups.iter().map(|g| g.locations.len()).sum();
        assert_eq!(total_refs, 4, "Should have 4 total references");
    }

    #[test]
    fn test_references_sorted_within_file() {
        let resp = json!([
            {"uri": "file:///src/a.rs", "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 5}}},
            {"uri": "file:///src/a.rs", "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 5}}},
            {"uri": "file:///src/a.rs", "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 5}}}
        ]);
        let groups = parse_references_response(resp);
        assert_eq!(groups.len(), 1);
        let locs = &groups[0].locations;
        assert_eq!(locs[0].line, 5);
        assert_eq!(locs[1].line, 10);
        assert_eq!(locs[2].line, 20);
    }
}

mod lsp_rename_tests {
    use ratterm::lsp::rename::{parse_prepare_rename, parse_workspace_edit, total_edit_count};
    use serde_json::json;

    #[test]
    fn test_rename_applies_changes_across_files() {
        let resp = json!({
            "changes": {
                "file:///src/main.rs": [
                    {"range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 12}}, "newText": "new_name"},
                    {"range": {"start": {"line": 10, "character": 8}, "end": {"line": 10, "character": 16}}, "newText": "new_name"}
                ],
                "file:///src/lib.rs": [
                    {"range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 8}}, "newText": "new_name"}
                ]
            }
        });
        let edit = parse_workspace_edit(resp).expect("Should parse workspace edit");
        assert_eq!(edit.changes.len(), 2, "Should have edits in 2 files");
        assert_eq!(total_edit_count(&edit), 3, "Should have 3 total edits");
    }

    #[test]
    fn test_prepare_rename_returns_range_and_placeholder() {
        let resp = json!({
            "range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 12}},
            "placeholder": "old_name"
        });
        let result = parse_prepare_rename(resp).expect("Should parse prepare rename");
        assert_eq!(result.placeholder, "old_name");
        assert_eq!(result.range.start_line, 5);
        assert_eq!(result.range.start_char, 4);
    }

    #[test]
    fn test_rename_with_document_changes() {
        let resp = json!({
            "documentChanges": [
                {
                    "textDocument": {"uri": "file:///src/main.rs", "version": 1},
                    "edits": [
                        {"range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}, "newText": "bar"}
                    ]
                }
            ]
        });
        let edit = parse_workspace_edit(resp).expect("Should parse document changes");
        assert_eq!(edit.changes.len(), 1);
    }
}

mod lsp_diagnostics_tests {
    use ratterm::lsp::diagnostics::{DiagnosticStore, DiagnosticSeverity, parse_diagnostics};
    use serde_json::json;

    #[test]
    fn test_diagnostics_underlines_at_correct_ranges() {
        let arr = vec![
            json!({
                "range": {"start": {"line": 0, "character": 5}, "end": {"line": 0, "character": 15}},
                "severity": 1,
                "message": "expected `;`",
                "code": "E0308",
                "source": "rustc"
            }),
            json!({
                "range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 8}},
                "severity": 2,
                "message": "unused variable `x`",
                "source": "rustc"
            })
        ];
        let diagnostics = parse_diagnostics(&arr);
        assert_eq!(diagnostics.len(), 2);

        // First diagnostic: error at line 0, chars 5-15
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].range.start_line, 0);
        assert_eq!(diagnostics[0].range.start_char, 5);
        assert_eq!(diagnostics[0].range.end_char, 15);
        assert_eq!(diagnostics[0].message, "expected `;`");

        // Second diagnostic: warning at line 3
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[1].range.start_line, 3);
    }

    #[test]
    fn test_diagnostic_store_update_and_query() {
        let store = DiagnosticStore::new();

        // Publish diagnostics
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}},
                    "severity": 1,
                    "message": "error here"
                },
                {
                    "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 10}},
                    "severity": 2,
                    "message": "warning here"
                }
            ]
        });
        store.update_from_notification(&notification);
        assert_eq!(store.total_count(), 2);

        // Update with empty diagnostics clears the file
        let clear_notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": []
        });
        store.update_from_notification(&clear_notification);
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn test_diagnostic_store_multiple_files() {
        let store = DiagnosticStore::new();

        store.update_from_notification(&json!({
            "uri": "file:///src/a.rs",
            "diagnostics": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}, "severity": 1, "message": "err1"}]
        }));
        store.update_from_notification(&json!({
            "uri": "file:///src/b.rs",
            "diagnostics": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}, "severity": 2, "message": "warn1"}]
        }));

        assert_eq!(store.total_count(), 2);
        let all = store.all();
        assert_eq!(all.len(), 2);
    }
}

mod lsp_code_actions_tests {
    use ratterm::lsp::actions::parse_code_actions;
    use serde_json::json;

    #[test]
    fn test_code_actions_parsed_with_titles() {
        let resp = json!([
            {
                "title": "Add missing import `std::io`",
                "kind": "quickfix",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [
                            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "use std::io;\n"}
                        ]
                    }
                }
            },
            {
                "title": "Extract to function",
                "kind": "refactor.extract",
                "isPreferred": false
            },
            {
                "title": "Generate impl block",
                "kind": "refactor"
            }
        ]);
        let actions = parse_code_actions(resp);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].title, "Add missing import `std::io`");
        assert!(actions[0].is_preferred);
        assert!(actions[0].edit.is_some());
        assert_eq!(actions[1].title, "Extract to function");
        assert!(!actions[1].is_preferred);
    }
}

mod lsp_symbols_tests {
    use ratterm::lsp::symbols::{parse_document_symbols, parse_workspace_symbols, flatten_symbols, SymbolKind};
    use serde_json::json;

    #[test]
    fn test_document_symbols_outline() {
        let resp = json!([
            {
                "name": "MyStruct",
                "kind": 23,
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                "selectionRange": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 12}},
                "children": [
                    {
                        "name": "new",
                        "kind": 12,
                        "range": {"start": {"line": 2, "character": 4}, "end": {"line": 5, "character": 5}},
                        "selectionRange": {"start": {"line": 2, "character": 11}, "end": {"line": 2, "character": 14}}
                    },
                    {
                        "name": "field_a",
                        "kind": 8,
                        "range": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 15}},
                        "selectionRange": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 11}}
                    }
                ]
            },
            {
                "name": "main",
                "kind": 12,
                "range": {"start": {"line": 12, "character": 0}, "end": {"line": 20, "character": 1}},
                "selectionRange": {"start": {"line": 12, "character": 3}, "end": {"line": 12, "character": 7}}
            }
        ]);

        let symbols = parse_document_symbols(resp);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[1].name, "main");
        assert_eq!(symbols[1].kind, SymbolKind::Function);

        // Flatten for display
        let flat = flatten_symbols(&symbols, 0);
        assert_eq!(flat.len(), 4); // MyStruct, new, field_a, main
        assert_eq!(flat[0].0, 0); // depth 0
        assert_eq!(flat[1].0, 1); // depth 1 (child of MyStruct)
        assert_eq!(flat[2].0, 1); // depth 1
        assert_eq!(flat[3].0, 0); // depth 0
    }

    #[test]
    fn test_workspace_symbols_search() {
        let resp = json!([
            {
                "name": "Config",
                "kind": 23,
                "location": {
                    "uri": "file:///src/config/mod.rs",
                    "range": {"start": {"line": 50, "character": 0}, "end": {"line": 100, "character": 1}}
                },
                "containerName": "config"
            },
            {
                "name": "Config::load",
                "kind": 6,
                "location": {
                    "uri": "file:///src/config/mod.rs",
                    "range": {"start": {"line": 60, "character": 4}, "end": {"line": 80, "character": 5}}
                },
                "containerName": "Config"
            }
        ]);

        let symbols = parse_workspace_symbols(resp);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Config");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].container_name.as_deref(), Some("config"));
    }
}

mod lsp_signature_tests {
    use ratterm::lsp::signature::parse_signature_help;
    use serde_json::json;

    #[test]
    fn test_signature_help_active_parameter() {
        let resp = json!({
            "signatures": [{
                "label": "fn foo(x: i32, y: &str, z: bool) -> Result<(), Error>",
                "parameters": [
                    {"label": "x: i32"},
                    {"label": "y: &str"},
                    {"label": "z: bool"}
                ],
                "documentation": "Does foo things."
            }],
            "activeSignature": 0,
            "activeParameter": 1
        });

        let result = parse_signature_help(resp).expect("Should parse signature help");
        assert_eq!(result.signatures.len(), 1);
        assert_eq!(result.active_parameter, 1);
        assert_eq!(result.signatures[0].parameters.len(), 3);
        assert_eq!(result.signatures[0].parameters[1].label, "y: &str");
    }

    #[test]
    fn test_signature_help_with_offset_labels() {
        let resp = json!({
            "signatures": [{
                "label": "fn bar(a: u8, b: String)",
                "parameters": [
                    {"label": [7, 12]},
                    {"label": [14, 23]}
                ]
            }],
            "activeSignature": 0,
            "activeParameter": 0
        });

        let result = parse_signature_help(resp).unwrap();
        let param = &result.signatures[0].parameters[0];
        assert_eq!(param.label_start, Some(7));
        assert_eq!(param.label_end, Some(12));
    }
}

mod lsp_formatting_tests {
    use ratterm::lsp::formatting::{parse_formatting_response, sort_edits_reverse, apply_edits_to_string};
    use serde_json::json;

    #[test]
    fn test_formatting_edits_applied() {
        let resp = json!([
            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "  "},
            {"range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}}, "newText": "    "}
        ]);

        let mut edits = parse_formatting_response(resp);
        assert_eq!(edits.len(), 2);

        sort_edits_reverse(&mut edits);
        assert_eq!(edits[0].range.start_line, 1); // Line 1 first (reverse order)
        assert_eq!(edits[1].range.start_line, 0);

        let content = "hello\nworld\n";
        let result = apply_edits_to_string(content, &edits);
        assert!(result.contains("    world")); // Indented
    }
}

mod lsp_config_tests {
    use ratterm::lsp::config::{detect_language, LspConfigRegistry};
    use std::path::Path;

    #[test]
    fn test_multi_language_detection() {
        assert_eq!(detect_language(Path::new("main.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(Path::new("app.py")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("index.tsx")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("styles.css")), Some("css".to_string()));
        assert_eq!(detect_language(Path::new("data.json")), Some("json".to_string()));
        assert_eq!(detect_language(Path::new("main.go")), Some("go".to_string()));
        assert_eq!(detect_language(Path::new("noext")), None);
    }

    #[test]
    fn test_config_registry_multi_server() {
        let registry = LspConfigRegistry::new();
        assert!(registry.get("rust").is_some());
        assert!(registry.get("python").is_some());
        assert!(registry.get("javascript").is_some());
        assert!(registry.get("typescript").is_some());
        assert!(registry.get("go").is_some());
        assert!(registry.get("c").is_some());
        assert!(registry.get("cpp").is_some());
    }
}
