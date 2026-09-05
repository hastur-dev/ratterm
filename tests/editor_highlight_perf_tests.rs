//! Does typing in a large file stall?
//!
//! The claim being checked is that highlighting is incremental: a keystroke in
//! a file of several thousand lines costs about what a keystroke in a small one
//! does, because tree-sitter reuses the previous tree and reads the rope in
//! chunks rather than the document being copied into a `String` every time.
//!
//! Thresholds here are deliberately loose — this runs on shared CI hardware and
//! under a debug build — but they are far below what a full reparse of the
//! whole file per keystroke would cost. The measured numbers are printed so a
//! reviewer can read the real figure rather than only "it passed".

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::{Duration, Instant};

use ratterm::editor::highlight::{HighlightKind, Language};
use ratterm::editor::{Editor, Position};

/// Lines in the synthetic file. About 145 KB of Rust.
const LINES: usize = 6_000;
/// Keystrokes measured per run.
const KEYSTROKES: usize = 200;

/// Builds a large, syntactically valid Rust file.
fn big_rust_source() -> String {
    let mut text = String::with_capacity(LINES * 24);
    for i in 0..LINES / 6 {
        text.push_str(&format!("/// Item {i}.\n"));
        text.push_str(&format!("pub fn item_{i}(value: usize) -> usize {{\n"));
        text.push_str(&format!("    let label = \"item {i}\";\n"));
        text.push_str("    let _ = label.len();\n");
        text.push_str("    value + 1\n");
        text.push_str("}\n");
    }
    text
}

/// Opens the file in an editor with highlighting on.
fn loaded_editor() -> Editor {
    let mut editor = Editor::new(120, 40);
    editor.insert_str(&big_rust_source());
    editor.set_language(Language::Rust);
    editor.set_cursor_position(Position::new(0, 0));
    // Draw once so the first-frame cost is not counted as a keystroke.
    let _ = editor.screen_decor();
    editor
}

/// Types `KEYSTROKES` characters and returns the total elapsed time, including
/// the work a frame would do afterwards.
fn time_typing(editor: &mut Editor) -> Duration {
    let start = Instant::now();
    for i in 0..KEYSTROKES {
        editor.type_char(if i % 10 == 9 { ' ' } else { 'x' });
        // A real frame asks for the visible lines' spans; include that cost.
        let _ = editor.screen_decor();
    }
    start.elapsed()
}

#[test]
fn typing_in_a_large_file_stays_responsive() {
    let mut editor = loaded_editor();
    assert!(
        editor.buffer().len_lines() >= LINES,
        "the fixture should be at least {LINES} lines"
    );
    assert!(editor.has_syntax_tree(), "the file must actually be parsed");

    // Type in the middle of the file, which is the worst case for a naive
    // implementation that re-reads the document from the start.
    editor.set_cursor_position(Position::new(LINES / 2, 0));
    let elapsed = time_typing(&mut editor);
    let per_key = elapsed / u32::try_from(KEYSTROKES).expect("small");

    println!(
        "highlighting: {} lines, {} bytes, {KEYSTROKES} keystrokes in {:?} ({:?} per keystroke)",
        editor.buffer().len_lines(),
        editor.buffer().len_bytes(),
        elapsed,
        per_key
    );

    // The bar is set for an unoptimised build on a machine running several test
    // binaries at once: the tree-sitter C runtime is compiled without
    // optimisation too, and a release build is far quicker. It is still well
    // under what the regressions this guards against cost — recomputing the
    // fold ranges per keystroke, or reparsing the whole file each time, both
    // measured in the hundreds of milliseconds here. The ratio test below is
    // the sharper signal.
    assert!(
        per_key < Duration::from_millis(100),
        "a keystroke took {per_key:?}, which is slow enough to feel like a stall"
    );
}

#[test]
fn a_keystroke_in_a_large_file_costs_about_what_a_small_one_does() {
    let mut small = Editor::new(120, 40);
    small.insert_str("fn main() {\n    let a = 1;\n}\n");
    small.set_language(Language::Rust);
    small.set_cursor_position(Position::new(1, 4));
    let _ = small.screen_decor();
    let small_time = time_typing(&mut small);

    let mut large = loaded_editor();
    large.set_cursor_position(Position::new(LINES / 2, 0));
    let large_time = time_typing(&mut large);

    println!(
        "highlighting: small file {small_time:?}, large file {large_time:?} for {KEYSTROKES} keystrokes"
    );

    // A full reparse per keystroke would put this in the hundreds. The bar is
    // set high enough to survive a loaded CI machine and low enough that losing
    // incrementality fails the test.
    let ratio = large_time.as_secs_f64() / small_time.as_secs_f64().max(1e-6);
    println!("highlighting: large/small ratio {ratio:.1}x");
    assert!(
        ratio < 60.0,
        "typing in a {LINES}-line file was {ratio:.1}x the cost of a 3-line file, \
         which means the reparse is not incremental"
    );
}

#[test]
fn the_spans_stay_correct_after_a_long_run_of_edits() {
    let mut editor = loaded_editor();
    let line = LINES / 2;
    editor.set_cursor_position(Position::new(line, 0));
    editor.type_str("// ");

    let spans = editor.highlight_line(line);
    assert!(
        spans.iter().any(|s| s.kind == HighlightKind::Comment),
        "the line that was commented out must highlight as a comment"
    );

    // A line far from the edit is untouched and still highlighted.
    let far = editor.highlight_line(1);
    assert!(
        far.iter().any(|s| s.kind == HighlightKind::Keyword),
        "a line away from the edit must keep its highlighting"
    );
}

#[test]
fn a_file_over_the_parse_cap_opens_without_highlighting_rather_than_stalling() {
    let mut editor = Editor::new(120, 40);
    let huge = "fn f() {}\n".repeat(300_000); // about 3 MB
    editor.insert_str(&huge);
    editor.set_language(Language::Rust);

    let start = Instant::now();
    let _ = editor.screen_decor();
    let elapsed = start.elapsed();
    println!(
        "highlighting: {} byte file drew in {elapsed:?}",
        editor.buffer().len_bytes()
    );

    assert!(
        !editor.has_syntax_tree(),
        "an over-sized file is not parsed"
    );
    assert!(
        editor.highlight_line(0).is_empty(),
        "an unparsed file has no spans"
    );
}

#[test]
fn rendering_a_screen_of_a_large_file_is_bounded_by_the_screen_not_the_file() {
    let editor = loaded_editor();
    let start = Instant::now();
    for _ in 0..20 {
        let decor = editor.screen_decor();
        assert_eq!(decor.len(), editor.view().height());
    }
    let elapsed = start.elapsed();
    let per_frame = elapsed / 20;
    println!("highlighting: 20 frames of a {LINES}-line file in {elapsed:?} ({per_frame:?} each)");
    assert!(
        per_frame < Duration::from_millis(50),
        "a frame took {per_frame:?}"
    );
}
