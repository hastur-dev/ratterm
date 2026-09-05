//! Key handling for the Kubernetes screens.
//!
//! Translation only: each key names an operation in
//! [`crate::app::k8s_ops`] or a state change on the manager. Nothing here
//! decides anything, so a key binding can be changed without touching
//! behaviour, and behaviour can be tested without pressing a key.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::k8s_manager::K8sView;

use super::App;

/// What a key press should cause.
///
/// Named rather than acted on directly so the mapping is a pure function that
/// a test can check exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K8sAction {
    /// Move the selection up.
    Up,
    /// Move the selection down.
    Down,
    /// Move to the first row.
    First,
    /// Move to the last row.
    Last,
    /// Connect to the selected context, or open the selected resource.
    Confirm,
    /// Go back a screen, or close if already at the first.
    Back,
    /// Close the manager.
    Close,
    /// Show the next resource kind.
    NextKind,
    /// Show the previous resource kind.
    PreviousKind,
    /// Reload the current listing.
    Refresh,
    /// Start typing a filter.
    StartFilter,
    /// Add a character to the filter.
    FilterChar(char),
    /// Remove the last filter character.
    FilterBackspace,
    /// Stop filtering, keeping what was typed.
    FilterDone,
    /// Clear the filter.
    FilterClear,
    /// Scale the selected deployment up.
    ScaleUp,
    /// Scale the selected deployment down.
    ScaleDown,
    /// Restart the selected deployment.
    Restart,
    /// Delete the selected pod.
    DeletePod,
    /// The key means nothing here.
    Ignored,
}

/// Decides what a key means on the Kubernetes screens.
///
/// `view` and `filtering` change the meaning of the same key, which is why
/// they are arguments rather than being read from the manager: it makes every
/// combination reachable from a test.
#[must_use]
pub fn action_for(key: KeyEvent, view: K8sView, filtering: bool) -> K8sAction {
    // While typing a filter, printable keys are text. Anything else would
    // make a filter containing "r" impossible to type.
    if filtering {
        return match key.code {
            KeyCode::Esc => K8sAction::FilterClear,
            KeyCode::Enter => K8sAction::FilterDone,
            KeyCode::Backspace => K8sAction::FilterBackspace,
            KeyCode::Up => K8sAction::Up,
            KeyCode::Down => K8sAction::Down,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                K8sAction::FilterChar(c)
            }
            _ => K8sAction::Ignored,
        };
    }

    match (view, key.code) {
        (_, KeyCode::Up | KeyCode::Char('k')) => K8sAction::Up,
        (_, KeyCode::Down | KeyCode::Char('j')) => K8sAction::Down,
        (_, KeyCode::Home | KeyCode::Char('g')) => K8sAction::First,
        (_, KeyCode::End | KeyCode::Char('G')) => K8sAction::Last,
        (_, KeyCode::Enter) => K8sAction::Confirm,
        (_, KeyCode::Esc) => K8sAction::Close,

        (K8sView::Contexts, _) => K8sAction::Ignored,

        (K8sView::Resources, KeyCode::Backspace) => K8sAction::Back,
        (K8sView::Resources, KeyCode::Tab) => K8sAction::NextKind,
        (K8sView::Resources, KeyCode::BackTab) => K8sAction::PreviousKind,
        (K8sView::Resources, KeyCode::Char('r')) => K8sAction::Refresh,
        (K8sView::Resources, KeyCode::Char('/')) => K8sAction::StartFilter,
        (K8sView::Resources, KeyCode::Char('+')) => K8sAction::ScaleUp,
        (K8sView::Resources, KeyCode::Char('-')) => K8sAction::ScaleDown,
        (K8sView::Resources, KeyCode::Char('R')) => K8sAction::Restart,
        (K8sView::Resources, KeyCode::Delete | KeyCode::Char('d')) => K8sAction::DeletePod,
        (K8sView::Resources, _) => K8sAction::Ignored,
    }
}

impl App {
    /// Handles a key while the Kubernetes screens are open.
    ///
    /// Returns true if the key was used.
    pub(super) fn handle_k8s_key(&mut self, key: KeyEvent) -> bool {
        let Some(manager) = self.k8s_manager.as_ref() else {
            return false;
        };

        let action = action_for(key, manager.view(), manager.is_filtering());
        if action == K8sAction::Ignored {
            // Consumed anyway: a stray key must not fall through to the editor
            // underneath a full-screen panel.
            return true;
        }

        self.apply_k8s_action(action);
        true
    }

    /// Carries out one action.
    fn apply_k8s_action(&mut self, action: K8sAction) {
        match action {
            K8sAction::Close => self.close_k8s_manager(),
            K8sAction::Refresh => self.k8s_refresh(),
            K8sAction::NextKind => self.k8s_next_kind(),
            K8sAction::PreviousKind => self.k8s_previous_kind(),
            K8sAction::ScaleUp => self.k8s_scale_selected(true),
            K8sAction::ScaleDown => self.k8s_scale_selected(false),
            K8sAction::Restart => self.k8s_restart_selected(),
            K8sAction::DeletePod => self.k8s_delete_selected_pod(),
            K8sAction::Back => self.k8s_show_contexts(),
            K8sAction::Confirm => self.confirm_k8s_selection(),
            other => self.apply_k8s_navigation(other),
        }
    }

    /// Handles the actions that only move within the current screen.
    fn apply_k8s_navigation(&mut self, action: K8sAction) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };

        let on_contexts = manager.view() == K8sView::Contexts;

        match action {
            K8sAction::Up => {
                if on_contexts {
                    manager.contexts_mut().select_previous();
                } else {
                    manager.select_previous();
                }
            }
            K8sAction::Down => {
                if on_contexts {
                    manager.contexts_mut().select_next();
                } else {
                    manager.select_next();
                }
            }
            K8sAction::First => {
                if on_contexts {
                    manager.contexts_mut().select_first();
                } else {
                    manager.select_first_row();
                }
            }
            K8sAction::Last => {
                if on_contexts {
                    manager.contexts_mut().select_last();
                } else {
                    manager.select_last_row();
                }
            }
            K8sAction::StartFilter => manager.start_filtering(),
            K8sAction::FilterChar(c) => manager.push_filter(c),
            K8sAction::FilterBackspace => manager.pop_filter(),
            K8sAction::FilterDone => manager.stop_filtering(),
            K8sAction::FilterClear => manager.clear_filter(),
            _ => {}
        }
    }

    /// Acts on Enter, which means different things on each screen.
    fn confirm_k8s_selection(&mut self) {
        let Some(manager) = self.k8s_manager.as_ref() else {
            return;
        };

        match manager.view() {
            K8sView::Contexts => self.k8s_connect_selected(),
            // On the resource screen there is nothing to open yet, so Enter
            // refreshes rather than doing nothing at all.
            K8sView::Resources => self.k8s_refresh(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    /// A plain key press.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn navigation_works_on_both_screens() {
        for view in [K8sView::Contexts, K8sView::Resources] {
            assert_eq!(action_for(key(KeyCode::Up), view, false), K8sAction::Up);
            assert_eq!(action_for(key(KeyCode::Down), view, false), K8sAction::Down);
            assert_eq!(
                action_for(key(KeyCode::Char('j')), view, false),
                K8sAction::Down,
                "vim keys work here too"
            );
            assert_eq!(action_for(key(KeyCode::Home), view, false), K8sAction::First);
            assert_eq!(action_for(key(KeyCode::End), view, false), K8sAction::Last);
        }
    }

    #[test]
    fn escape_closes_from_either_screen() {
        assert_eq!(
            action_for(key(KeyCode::Esc), K8sView::Contexts, false),
            K8sAction::Close
        );
        assert_eq!(
            action_for(key(KeyCode::Esc), K8sView::Resources, false),
            K8sAction::Close
        );
    }

    #[test]
    fn backspace_goes_back_a_screen_only_where_there_is_one() {
        assert_eq!(
            action_for(key(KeyCode::Backspace), K8sView::Resources, false),
            K8sAction::Back
        );
        assert_eq!(
            action_for(key(KeyCode::Backspace), K8sView::Contexts, false),
            K8sAction::Ignored,
            "there is nothing behind the context list"
        );
    }

    #[test]
    fn the_resource_keys_do_nothing_on_the_context_screen() {
        // Pressing 'r' while choosing a context should not refresh a listing
        // that is not showing.
        for code in [
            KeyCode::Char('r'),
            KeyCode::Char('/'),
            KeyCode::Tab,
            KeyCode::Char('+'),
        ] {
            assert_eq!(
                action_for(key(code), K8sView::Contexts, false),
                K8sAction::Ignored,
                "{code:?}"
            );
        }
    }

    #[test]
    fn the_resource_keys_work_on_the_resource_screen() {
        let view = K8sView::Resources;
        assert_eq!(action_for(key(KeyCode::Tab), view, false), K8sAction::NextKind);
        assert_eq!(
            action_for(key(KeyCode::BackTab), view, false),
            K8sAction::PreviousKind
        );
        assert_eq!(
            action_for(key(KeyCode::Char('r')), view, false),
            K8sAction::Refresh
        );
        assert_eq!(
            action_for(key(KeyCode::Char('/')), view, false),
            K8sAction::StartFilter
        );
        assert_eq!(
            action_for(key(KeyCode::Char('+')), view, false),
            K8sAction::ScaleUp
        );
        assert_eq!(
            action_for(key(KeyCode::Char('-')), view, false),
            K8sAction::ScaleDown
        );
        assert_eq!(
            action_for(key(KeyCode::Char('R')), view, false),
            K8sAction::Restart
        );
        assert_eq!(
            action_for(key(KeyCode::Delete), view, false),
            K8sAction::DeletePod
        );
    }

    #[test]
    fn a_filter_can_contain_the_letters_that_are_otherwise_commands() {
        // Without this, a pod called "redis" could not be filtered for.
        let view = K8sView::Resources;
        for c in ['r', 'j', 'k', 'd', 'g', 'R', '+', '-'] {
            assert_eq!(
                action_for(key(KeyCode::Char(c)), view, true),
                K8sAction::FilterChar(c),
                "typing {c} while filtering"
            );
        }
    }

    #[test]
    fn filtering_keeps_the_navigation_keys_that_have_no_character() {
        let view = K8sView::Resources;
        assert_eq!(action_for(key(KeyCode::Up), view, true), K8sAction::Up);
        assert_eq!(action_for(key(KeyCode::Down), view, true), K8sAction::Down);
    }

    #[test]
    fn escape_while_filtering_clears_rather_than_closing() {
        // Closing the whole panel on Esc would lose the listing as well.
        assert_eq!(
            action_for(key(KeyCode::Esc), K8sView::Resources, true),
            K8sAction::FilterClear
        );
    }

    #[test]
    fn enter_while_filtering_finishes_the_filter() {
        assert_eq!(
            action_for(key(KeyCode::Enter), K8sView::Resources, true),
            K8sAction::FilterDone
        );
    }

    #[test]
    fn backspace_while_filtering_edits_the_text() {
        assert_eq!(
            action_for(key(KeyCode::Backspace), K8sView::Resources, true),
            K8sAction::FilterBackspace
        );
    }

    #[test]
    fn a_control_combination_is_not_typed_into_the_filter() {
        let mut event = key(KeyCode::Char('c'));
        event.modifiers = KeyModifiers::CONTROL;
        assert_eq!(
            action_for(event, K8sView::Resources, true),
            K8sAction::Ignored,
            "ctrl+c should not appear in the filter box"
        );
    }

    #[test]
    fn enter_confirms_on_both_screens() {
        assert_eq!(
            action_for(key(KeyCode::Enter), K8sView::Contexts, false),
            K8sAction::Confirm
        );
        assert_eq!(
            action_for(key(KeyCode::Enter), K8sView::Resources, false),
            K8sAction::Confirm
        );
    }

    #[test]
    fn an_unmapped_key_is_ignored_rather_than_misread() {
        assert_eq!(
            action_for(key(KeyCode::F(9)), K8sView::Resources, false),
            K8sAction::Ignored
        );
    }
}
