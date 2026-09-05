# The editor

Ratterm's editor is an API layer with a thin renderer on top. Every behaviour a
user can trigger is a method on `Editor` in `src/editor/`, tested without a
terminal; `src/app/input_editor.rs` translates key events into those calls and
`src/ui/editor_widget/` paints what they produce. Neither the input layer nor
the widget decides anything.

## Layout

| File | What lives there |
|---|---|
| `editor/mod.rs` | The `Editor` struct, its accessors, `EditorMode` |
| `editor/document.rs` | Opening, saving, and swapping documents between tabs |
| `editor/text_ops.rs` | The primitive mutations, and where they tell the syntax tree |
| `editor/buffer.rs`, `buffer/query.rs` | The rope, undo history, and read-only queries over it |
| `editor/language.rs` | `Language`, `HighlightKind`, `HighlightSpan` |
| `editor/highlight.rs` | The tree-sitter `Highlighter` |
| `editor/highlight_edit.rs` | Buffer edits translated into tree-sitter `InputEdit`s |
| `editor/syntax.rs` | The tree the editor holds, and when it is reparsed |
| `editor/brackets.rs`, `bracket_scan.rs` | Auto-pairing rules, and the scan that pairs brackets |
| `editor/indent.rs`, `indent_ops.rs` | Indent detection, and applying it on Enter and on `}` |
| `editor/fold.rs`, `fold_ops.rs` | Where regions are, which are collapsed, and what a frame draws |
| `editor/search.rs`, `search_ops.rs` | The find bar's state, and driving it |
| `editor/multicursor.rs`, `multicursor_ops.rs` | Secondary cursors and the fan-out arithmetic |
| `editor/vim/` | The key-sequence parser, motions, text objects, registers, and the executor |
| `editor/emacs.rs`, `emacs_commands.rs`, `emacs_keys.rs`, `emacs_exec.rs` | Kill ring, mark, `M-x` table, key map, and the executor |
| `editor/decor.rs` | What the renderer needs to know about a line |
| `editor/typing.rs` | What one printable keystroke does |

## Syntax highlighting

Grammars for Rust, Python and JavaScript ship with the crate. Highlighting is
driven by each grammar's own highlight query rather than a hand-written table,
so capture names map onto a small renderer-agnostic set (`HighlightKind`) and
the widget picks a colour for each kind.

Two things keep it cheap enough to run on every keystroke in a large file.

**The document is never copied to parse it.** `Highlighter::parse` feeds
tree-sitter a callback that reads chunks straight out of the rope
(`Buffer::chunk_at_byte`). Materialising a multi-megabyte `String` per keystroke
was the obvious cost, and it is not paid.

**The reparse is deferred and incremental.** Every mutation goes through
`Editor::apply_insert` or `apply_delete`, which builds the matching `InputEdit`
*before* the text moves and hands it to `Syntax::note_edit`. That records the
edit against the tree and marks a reparse owed; the parse itself happens on the
first request for spans, which is once per frame. A compound edit — replace-all
over a hundred matches, a fan-out to twenty cursors — therefore costs one
reparse rather than one per sub-edit.

Anything that reaches the buffer another way (`buffer_mut()`, the IPC API) is
caught by `Buffer::revision`, a counter bumped by every mutation. When the
revision the tree was built from no longer matches, the tree is thrown away and
reparsed from scratch rather than drawn with spans in the wrong places.

Files at or above `MAX_HIGHLIGHT_BYTES` (2 MB) are not parsed at all; they open
without colour instead of stalling.

Measured on this machine over a 6 000-line (124 KB) Rust file, 200 keystrokes
typed into the middle of it with a frame drawn after each:

| | Debug | Release |
|---|---|---|
| Per keystroke, first cut (folds recomputed and tree reparsed eagerly) | 165 ms | — |
| Per keystroke, current | 19.2 ms | 15.2 ms |
| Drawing one 40-line frame with nothing to reparse | 0.26 ms | 10.7 µs |

Almost all of what remains is the one incremental tree-sitter reparse per frame.
A parse from scratch of the same file costs about nine times that, and the
fixture is a deliberately awkward shape — a thousand small top-level items, so
every reuse boundary has to be checked. `tests/editor_highlight_perf_tests.rs`
measures all of this and prints the figures.

## Folding

`compute_folds` derives the regions from the text: brace pairs where the
language has them, indentation where it does not, plus runs of line comments.
`FoldState` owns which of those the user collapsed, keyed by start line, so an
inner region keeps its own state while an outer one is collapsed over it.

Recomputing the regions is a whole-file scan, so it does not run per keystroke.
`refresh_folds_if_needed` runs it when the line count changes — typing inside a
line cannot move a boundary — or when anything is currently collapsed, where a
stale range would hide the wrong lines. Fold commands call `ensure_folds` first.

`Editor::screen_lines` returns the buffer lines a frame draws, skipping hidden
ones, so a collapsed region occupies exactly one row. The cursor cannot stand
inside a collapsed region: `clamp_cursor_out_of_folds` moves it to the line that
is still on screen, and `move_up_visible` / `move_down_visible` step over
regions rather than into them.

## Auto-indentation and brackets

`indent_for_new_line` decides what a new line starts with: one level deeper
after an opening bracket or a Python `:`, one level shallower when a closing
bracket will begin the new line, and otherwise the current line's indent. The
unit comes from `detect_indent`, which reads the file's own habits rather than
assuming four spaces.

`insert_newline_smart` additionally splits a bracket pair: pressing Enter
between `{` and `}` leaves an indented blank line with the brace below it.

Typing goes through `Editor::type_char`, which asks `brackets::type_action` what
to do: insert the character, insert it with its partner, or step over a closer
that is already there. Typing a closing bracket also re-indents the line onto
its opener. Auto-pairing is suppressed directly before a word character, and
quotes are also suppressed directly after one so an apostrophe in prose does not
grow a partner. `Editor::set_auto_pair` turns any of it off.

`matching_bracket_pair` reports the pair to highlight: the bracket under the
cursor wins, and failing that the one immediately before it, so a cursor just
past a closing brace still highlights both.

## Vim

`VimState::feed` takes one key and answers `Pending`, `Command`, or `Rejected`.
It parses counts (before and after an operator, which multiply, so `2d3w` is a
delete over six words), operators, motions, text objects, registers, marks,
`f`/`t` searches with `;` and `,`, `.` repeat, and the `:` command line.
`Editor::feed_vim_key` runs the result.

Visual mode reuses the same parser: a motion extends the selection, and an
operator applies to the selection as soon as it is typed rather than waiting for
a target (`VimState::take_visual_operator`).

`:` commands the editor cannot finish itself come back as a `VimEffect` —
`Save`, `Quit`, `SaveAndQuit`, `QuitWithoutSaving`, or `Ex(..)` — which
`input_editor.rs` carries out. `:s` and `:%s` are parsed and applied by the
editor.

## Emacs

`emacs.rs` owns the kill ring and the mark; `emacs_commands.rs` owns the `M-x`
name table; `emacs_keys.rs` maps chords, including the `C-x` prefix, onto
commands; `emacs_exec.rs` applies them. Consecutive kills join into one ring
entry, which is what makes repeated `C-k` accumulate a region, and any command
that is not a kill breaks the run.

Commands the editor cannot finish itself come back as an `EmacsEffect`.

## Search and replace

`SearchState` owns the query, the replacement, which field has focus, and the
match list. `Editor::feed_search_key` drives it. Navigation wraps at both ends
and records that it wrapped so the bar can say so; the count (`3/12`) is always
shown once there is a query. Replace-one and replace-all are each a single undo
step, and replace-all applies from the end of the document so earlier positions
stay valid.

The bar is drawn by `ui/editor_widget/search_bar.rs` along the bottom row of the
editor pane; its text is assembled by `bar_text`, a pure function of the state.

## Multiple cursors

The primary cursor keeps selection and preferred column; secondary cursors are
positions in `MultiCursor`. Typing fans out to all of them as one undo group.
The arithmetic — where each cursor ends up after an insertion or a backspace at
every one of them — is in `multicursor.rs` as pure functions.

Auto-pairing is skipped while secondary cursors are active, because the answer
can differ per cursor and a fan-out has to be one uniform edit.

## Rendering

`Editor::screen_decor` returns one `LineDecor` per screen row: the syntax spans,
the non-plain character roles (selected, matched bracket, search hit, current
search hit, secondary cursor), and how many lines a fold at that row hides.
Roles are ordered by priority, so a keyword inside the selection stays a
keyword and gains a background.

The widget walks characters and paints cells, handling tabs, wide characters and
horizontal scrolling, and nothing else. Because the model is a plain value,
highlighting and folding can be asserted without a terminal — and the rendered
cells can be asserted with one, through `ratatui::backend::TestBackend`.

## Tests

| Command | Covers |
|---|---|
| `cargo test --lib editor::` | Every module's own unit tests |
| `cargo test --test editor_render_tests` | Highlighting, folding, brackets and dirty markers in rendered cells |
| `cargo test --test editor_modal_tests` | Vim and Emacs end to end |
| `cargo test --test editor_search_tests` | Search, replace, and wrap-around |
| `cargo test --test editor_multicursor_tests` | Multiple cursors, auto-indent, auto-pairing |
| `cargo test --test editor_highlight_perf_tests` | Highlighting cost, with the numbers printed |
