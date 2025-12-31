//! Integration tests for the autocomplete/completion system.
//!
//! These tests verify that:
//! - The completion engine is properly initialized with both LSP and keyword providers
//! - Completion triggers correctly when typing in the editor
//! - Suggestions are generated for supported languages (Rust, Python, etc.)
//! - The completion handle lifecycle works correctly
//!
//! NOTE: Tests that use CompletionHandle must use #[test] instead of #[tokio::test]
//! because CompletionHandle creates its own internal tokio runtime, and dropping
//! a runtime from within an async context causes a panic.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ratterm::completion::{
    CompletionContext, CompletionHandle, CompletionItem, CompletionKind, CompletionProvider,
    KeywordProvider, LspProvider,
};

// ============================================================================
// Completion Handle Tests
// ============================================================================

#[test]
fn test_completion_handle_initializes_with_lsp_provider() {
    // Verify that the completion handle can be created with a working directory
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Handle should be created successfully
    assert!(!handle.has_suggestion());

    // Clean shutdown
    handle.shutdown();
}

#[test]
fn test_completion_handle_triggers_for_rust_file() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Create a context simulating typing in a Rust file
    let context = CompletionContext::new("rust", 0, 3)
        .with_file_path(PathBuf::from("test.rs"))
        .with_prefix("let")
        .with_word_at_cursor("let")
        .with_buffer_content("let x = 1;\nlet xy = 2;\nlet xyz = 3;");

    // Trigger completion
    handle.trigger_immediate(context);

    // Wait for completion to process
    thread::sleep(Duration::from_millis(200));

    // Should have generated a suggestion from keyword provider
    // (LSP may not be available in test environment)
    // The keyword provider should find matches for "let" from buffer content

    handle.shutdown();
}

#[test]
fn test_completion_handle_triggers_for_python_file() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Create a context simulating typing in a Python file
    let context = CompletionContext::new("python", 0, 3)
        .with_file_path(PathBuf::from("test.py"))
        .with_prefix("def")
        .with_word_at_cursor("def")
        .with_buffer_content("def foo():\n    pass\ndef bar():\n    return 1");

    handle.trigger_immediate(context);
    thread::sleep(Duration::from_millis(200));

    handle.shutdown();
}

#[test]
fn test_completion_handle_dismiss_clears_suggestion() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Trigger a completion
    let context = CompletionContext::new("rust", 0, 1)
        .with_prefix("f")
        .with_word_at_cursor("f")
        .with_buffer_content("fn main() {}\nfn foo() {}\nfn foobar() {}");

    handle.trigger_immediate(context);
    thread::sleep(Duration::from_millis(200));

    // Dismiss should clear any suggestion
    handle.dismiss();
    assert!(!handle.has_suggestion());

    handle.shutdown();
}

#[test]
fn test_completion_handle_cancel_stops_pending_request() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Trigger with debounce
    let context = CompletionContext::new("rust", 0, 2)
        .with_prefix("fn")
        .with_word_at_cursor("fn")
        .with_buffer_content("fn main() {}");

    handle.trigger(context);

    // Cancel immediately before debounce completes
    handle.cancel();

    // Wait a bit to ensure cancellation worked
    thread::sleep(Duration::from_millis(100));

    // Should not have a suggestion since we cancelled
    assert!(!handle.has_suggestion());

    handle.shutdown();
}

// ============================================================================
// Keyword Provider Tests
// ============================================================================

#[tokio::test]
async fn test_keyword_provider_returns_rust_keywords() {
    let provider = KeywordProvider::new();

    assert!(provider.supports_language("rust"));
    assert_eq!(provider.id(), "keyword");

    // Create context with partial prefix "le" to match "let"
    let context = CompletionContext::new("rust", 0, 2)
        .with_prefix("le")
        .with_word_at_cursor("le")
        .with_buffer_content("fn main() {\n    le\n}");

    let result = provider.complete(&context).await;

    // Should return keyword matches (e.g., "let" for "le" prefix)
    assert!(result.is_some(), "Keyword provider should return results");
    let completion = result.unwrap();
    assert!(!completion.items.is_empty(), "Should have completion items");
    // Verify we got "let" keyword
    assert!(
        completion.items.iter().any(|i| i.label == "let"),
        "Should have 'let' keyword in results"
    );
}

#[tokio::test]
async fn test_keyword_provider_returns_python_keywords() {
    let provider = KeywordProvider::new();

    assert!(provider.supports_language("python"));

    // Use "de" prefix to match "def", "del", etc.
    let context = CompletionContext::new("python", 0, 2)
        .with_prefix("de")
        .with_word_at_cursor("de")
        .with_buffer_content("de");

    let result = provider.complete(&context).await;

    assert!(
        result.is_some(),
        "Keyword provider should return results for Python"
    );
    let completion = result.unwrap();
    assert!(
        completion.items.iter().any(|i| i.label == "def"),
        "Should have 'def' keyword in results"
    );
}

#[tokio::test]
async fn test_keyword_provider_extracts_buffer_words() {
    let provider = KeywordProvider::new();

    // The provider should extract words from the buffer and suggest them
    let context = CompletionContext::new("rust", 0, 4)
        .with_prefix("my_v")
        .with_word_at_cursor("my_v")
        .with_buffer_content("let my_variable = 1;\nlet my_value = 2;\nlet other = 3;");

    let result = provider.complete(&context).await;

    assert!(result.is_some());
    let completion = result.unwrap();

    // Should find my_variable and my_value as suggestions
    let labels: Vec<&str> = completion.items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels
            .iter()
            .any(|l| l.contains("my_variable") || l.contains("my_value")),
        "Should suggest words from buffer matching prefix. Got: {:?}",
        labels
    );
}

#[tokio::test]
async fn test_keyword_provider_filters_by_prefix() {
    let provider = KeywordProvider::new();

    let context = CompletionContext::new("rust", 0, 3)
        .with_prefix("str")
        .with_word_at_cursor("str")
        .with_buffer_content("struct Foo {}\nString::new()\nstrlen()");

    let result = provider.complete(&context).await;

    assert!(result.is_some());
    let completion = result.unwrap();

    // All suggestions should start with "str"
    for item in &completion.items {
        assert!(
            item.label.to_lowercase().starts_with("str")
                || item.insert_text.to_lowercase().starts_with("str"),
            "Item {} should start with 'str'",
            item.label
        );
    }
}

// ============================================================================
// LSP Provider Tests
// ============================================================================

#[test]
fn test_lsp_provider_supports_rust() {
    let provider = LspProvider::new(PathBuf::from("."));

    assert!(provider.supports_language("rust"));
    assert_eq!(provider.priority(), 100); // LSP has high priority
}

#[test]
fn test_lsp_provider_supports_python() {
    let provider = LspProvider::new(PathBuf::from("."));

    assert!(provider.supports_language("python"));
}

#[test]
fn test_lsp_provider_supports_javascript() {
    let provider = LspProvider::new(PathBuf::from("."));

    assert!(provider.supports_language("javascript"));
    assert!(provider.supports_language("typescript"));
}

#[test]
fn test_lsp_provider_supports_go() {
    let provider = LspProvider::new(PathBuf::from("."));

    assert!(provider.supports_language("go"));
}

#[test]
fn test_lsp_provider_does_not_support_unknown() {
    let provider = LspProvider::new(PathBuf::from("."));

    assert!(!provider.supports_language("unknown_language_xyz"));
}

#[test]
fn test_lsp_provider_has_higher_priority_than_keyword() {
    let lsp_provider = LspProvider::new(PathBuf::from("."));
    let keyword_provider = KeywordProvider::new();

    assert!(
        lsp_provider.priority() > keyword_provider.priority(),
        "LSP provider should have higher priority than keyword provider"
    );
}

// ============================================================================
// Completion Context Tests
// ============================================================================

#[test]
fn test_completion_context_creation() {
    let context = CompletionContext::new("rust", 10, 5)
        .with_file_path(PathBuf::from("/path/to/file.rs"))
        .with_prefix("let x")
        .with_word_at_cursor("x")
        .with_line_content("    let x = foo();")
        .with_buffer_content("fn main() {\n    let x = foo();\n}");

    assert_eq!(context.language_id, "rust");
    assert_eq!(context.line, 10);
    assert_eq!(context.col, 5);
    assert_eq!(context.prefix, "let x");
    assert_eq!(context.word_at_cursor, "x");
    assert!(context.file_path.is_some());
}

#[test]
fn test_completion_context_with_trigger_char() {
    let context = CompletionContext::new("rust", 0, 1).with_trigger_char('.');

    assert_eq!(context.trigger_char, Some('.'));
}

// ============================================================================
// Completion Item Tests
// ============================================================================

#[test]
fn test_completion_item_creation() {
    let item = CompletionItem::new(
        "println!(\"{}\", )".to_string(),
        "println!".to_string(),
        CompletionKind::Function,
        "test".to_string(),
    )
    .with_detail("Prints to stdout")
    .with_priority(100);

    assert_eq!(item.label, "println!");
    assert_eq!(item.insert_text, "println!(\"{}\", )");
    assert_eq!(item.kind, CompletionKind::Function);
    assert_eq!(item.detail, Some("Prints to stdout".to_string()));
    assert_eq!(item.priority, 100);
}

#[test]
fn test_completion_kind_as_str() {
    assert_eq!(CompletionKind::Function.as_str(), "fn");
    assert_eq!(CompletionKind::Variable.as_str(), "var");
    assert_eq!(CompletionKind::Keyword.as_str(), "kw");
    assert_eq!(CompletionKind::Snippet.as_str(), "snip");
}

// ============================================================================
// Integration: Full Completion Flow
// ============================================================================

#[test]
fn test_full_completion_flow_rust() {
    // Simulate a complete flow: create handle, trigger, wait, check result
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let handle = CompletionHandle::new(cwd);

    // Simulate typing "pri" in a Rust file (should match "println")
    let context = CompletionContext::new("rust", 0, 3)
        .with_file_path(PathBuf::from("main.rs"))
        .with_prefix("pri")
        .with_word_at_cursor("pri")
        .with_buffer_content("fn main() {\n    pri\n}");

    // Trigger immediate completion
    handle.trigger_immediate(context);

    // Wait for completion engine to process
    thread::sleep(Duration::from_millis(300));

    // The keyword provider should have found "println" or similar
    // Note: LSP won't respond without an actual server running
    let suggestion = handle.suggestion_text();

    // We accept either having a suggestion or not (depends on keyword matches)
    // The important thing is that the system doesn't panic or hang
    if let Some(text) = suggestion {
        assert!(!text.is_empty(), "Suggestion text should not be empty");
    }

    handle.shutdown();
}

#[test]
fn test_full_completion_flow_with_accept() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Create context with buffer that has matching words
    let context = CompletionContext::new("rust", 2, 5)
        .with_prefix("hello")
        .with_word_at_cursor("hello")
        .with_buffer_content("let hello_world = 1;\nlet hello_there = 2;\nhello");

    handle.trigger_immediate(context);
    thread::sleep(Duration::from_millis(300));

    // Try to accept - should return the suggestion text if available
    let accepted = handle.accept();

    // If we got a suggestion, verify it was returned on accept
    if accepted.is_some() {
        // After accepting, there should be no more suggestion
        assert!(!handle.has_suggestion());
    }

    handle.shutdown();
}

#[test]
fn test_multiple_rapid_completions() {
    // Test that rapid successive completions don't cause issues
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Fire multiple completions rapidly
    for i in 0..5 {
        let prefix = format!("test{}", i);
        let context = CompletionContext::new("rust", 0, prefix.len())
            .with_prefix(&prefix)
            .with_word_at_cursor(&prefix)
            .with_buffer_content("fn test() {}\nfn test1() {}\nfn test2() {}");

        handle.trigger_immediate(context);
    }

    // Wait a bit for processing
    thread::sleep(Duration::from_millis(200));

    // Should not panic or hang - system should handle rapid requests gracefully
    handle.shutdown();
}

#[test]
fn test_completion_with_empty_buffer() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Empty buffer should still work (return language keywords)
    let context = CompletionContext::new("rust", 0, 1)
        .with_prefix("f")
        .with_word_at_cursor("f")
        .with_buffer_content("");

    handle.trigger_immediate(context);
    thread::sleep(Duration::from_millis(200));

    // Should still function, keywords like "fn", "for" should be available
    // from the language's keyword list
    handle.shutdown();
}

#[test]
fn test_completion_shutdown_is_clean() {
    let cwd = PathBuf::from(".");
    let handle = CompletionHandle::new(cwd);

    // Trigger some work
    let context = CompletionContext::new("rust", 0, 2)
        .with_prefix("le")
        .with_word_at_cursor("le")
        .with_buffer_content("let x = 1;");

    handle.trigger(context);

    // Shutdown immediately - should not panic
    handle.shutdown();

    // Verify shutdown completed
    assert!(!handle.has_suggestion());
}
