//! The Emacs command vocabulary and its `M-x` name table.
//!
//! Split out of `emacs.rs` so neither file grows past the project's size limit;
//! [`emacs`](super::emacs) re-exports everything here.

use std::sync::LazyLock;

/// A command an `M-x` prompt or a key binding can invoke.
///
/// These are descriptions, not actions: the input layer decides which editor
/// calls each one turns into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmacsCommand {
    /// `C-a`
    MoveBeginningOfLine,
    /// `C-e`
    MoveEndOfLine,
    /// `C-f`
    ForwardChar,
    /// `C-b`
    BackwardChar,
    /// `C-n`
    NextLine,
    /// `C-p`
    PreviousLine,
    /// `M-f`
    ForwardWord,
    /// `M-b`
    BackwardWord,
    /// `M-<`
    BeginningOfBuffer,
    /// `M->`
    EndOfBuffer,
    /// `M-g g`
    GotoLine,
    /// `C-v`
    ScrollUpCommand,
    /// `M-v`
    ScrollDownCommand,
    /// `C-l`
    RecenterTopBottom,
    /// `C-k`
    KillLine,
    /// `M-d`
    KillWord,
    /// `M-DEL`
    BackwardKillWord,
    /// `C-w`
    KillRegion,
    /// `M-w`
    KillRingSave,
    /// `C-y`
    Yank,
    /// `M-y`
    YankPop,
    /// `C-SPC`
    SetMarkCommand,
    /// `C-x C-x`
    ExchangePointAndMark,
    /// `C-x h`
    MarkWholeBuffer,
    /// `C-d`
    DeleteChar,
    /// `DEL`
    DeleteBackwardChar,
    /// `C-t`
    TransposeChars,
    /// `C-o`
    OpenLine,
    /// `C-j`
    NewlineAndIndent,
    /// `RET`
    Newline,
    /// `TAB`
    IndentForTabCommand,
    /// `M-;`
    CommentDwim,
    /// `C-_`
    Undo,
    /// `C-s`
    IsearchForward,
    /// `C-r`
    IsearchBackward,
    /// `M-%`
    QueryReplace,
    /// `C-x C-s`
    SaveBuffer,
    /// `C-x C-f`
    FindFile,
    /// `C-x C-c`
    SaveBuffersKillTerminal,
    /// `C-g`
    KeyboardQuit,
    /// `C-x =`
    WhatCursorPosition,
}

/// The `M-x` name table, in the order a completion list should show it.
const COMMANDS: &[(&str, EmacsCommand)] = &[
    ("backward-char", EmacsCommand::BackwardChar),
    ("backward-kill-word", EmacsCommand::BackwardKillWord),
    ("backward-word", EmacsCommand::BackwardWord),
    ("beginning-of-buffer", EmacsCommand::BeginningOfBuffer),
    ("comment-dwim", EmacsCommand::CommentDwim),
    ("delete-backward-char", EmacsCommand::DeleteBackwardChar),
    ("delete-char", EmacsCommand::DeleteChar),
    ("end-of-buffer", EmacsCommand::EndOfBuffer),
    (
        "exchange-point-and-mark",
        EmacsCommand::ExchangePointAndMark,
    ),
    ("find-file", EmacsCommand::FindFile),
    ("forward-char", EmacsCommand::ForwardChar),
    ("forward-word", EmacsCommand::ForwardWord),
    ("goto-line", EmacsCommand::GotoLine),
    ("indent-for-tab-command", EmacsCommand::IndentForTabCommand),
    ("isearch-backward", EmacsCommand::IsearchBackward),
    ("isearch-forward", EmacsCommand::IsearchForward),
    ("keyboard-quit", EmacsCommand::KeyboardQuit),
    ("kill-line", EmacsCommand::KillLine),
    ("kill-region", EmacsCommand::KillRegion),
    ("kill-ring-save", EmacsCommand::KillRingSave),
    ("kill-word", EmacsCommand::KillWord),
    ("mark-whole-buffer", EmacsCommand::MarkWholeBuffer),
    ("move-beginning-of-line", EmacsCommand::MoveBeginningOfLine),
    ("move-end-of-line", EmacsCommand::MoveEndOfLine),
    ("newline", EmacsCommand::Newline),
    ("newline-and-indent", EmacsCommand::NewlineAndIndent),
    ("next-line", EmacsCommand::NextLine),
    ("open-line", EmacsCommand::OpenLine),
    ("previous-line", EmacsCommand::PreviousLine),
    ("query-replace", EmacsCommand::QueryReplace),
    ("recenter-top-bottom", EmacsCommand::RecenterTopBottom),
    ("save-buffer", EmacsCommand::SaveBuffer),
    (
        "save-buffers-kill-terminal",
        EmacsCommand::SaveBuffersKillTerminal,
    ),
    ("scroll-down-command", EmacsCommand::ScrollDownCommand),
    ("scroll-up-command", EmacsCommand::ScrollUpCommand),
    ("set-mark-command", EmacsCommand::SetMarkCommand),
    ("transpose-chars", EmacsCommand::TransposeChars),
    ("undo", EmacsCommand::Undo),
    ("what-cursor-position", EmacsCommand::WhatCursorPosition),
    ("yank", EmacsCommand::Yank),
    ("yank-pop", EmacsCommand::YankPop),
];

static COMMAND_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| COMMANDS.iter().map(|(name, _)| *name).collect());

/// Returns every `M-x` command name, sorted.
#[must_use]
pub fn command_names() -> &'static [&'static str] {
    &COMMAND_NAMES
}

/// Looks up a command by its `M-x` name.
///
/// Matching is exact; an `M-x` prompt filters with [`command_names`] before
/// resolving.
#[must_use]
pub fn resolve(name: &str) -> Option<EmacsCommand> {
    COMMANDS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, command)| *command)
}

/// Returns every command name starting with `prefix`, in table order.
///
/// This is what an `M-x` prompt shows while the user types.
#[must_use]
pub fn complete(prefix: &str) -> Vec<&'static str> {
    COMMAND_NAMES
        .iter()
        .copied()
        .filter(|name| name.starts_with(prefix))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn known_command_names_resolve() {
        assert_eq!(resolve("kill-line"), Some(EmacsCommand::KillLine));
        assert_eq!(resolve("yank-pop"), Some(EmacsCommand::YankPop));
        assert_eq!(
            resolve("exchange-point-and-mark"),
            Some(EmacsCommand::ExchangePointAndMark)
        );
    }

    #[test]
    fn unknown_command_names_do_not_resolve() {
        assert_eq!(resolve("frobnicate"), None);
        assert_eq!(resolve(""), None);
        assert_eq!(resolve("Kill-Line"), None);
    }

    #[test]
    fn the_name_table_is_sorted_unique_and_fully_resolvable() {
        let names = command_names();
        assert!(!names.is_empty());
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.as_slice(), names, "names must be sorted and unique");
        for name in names {
            assert!(resolve(name).is_some(), "{name} does not resolve");
        }
    }

    #[test]
    fn completion_narrows_by_prefix() {
        let hits = complete("kill-");
        assert!(hits.contains(&"kill-line"));
        assert!(hits.contains(&"kill-region"));
        assert!(!hits.contains(&"yank"));
        assert_eq!(complete("").len(), command_names().len());
        assert!(complete("zzz").is_empty());
    }
}
