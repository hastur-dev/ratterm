//! Tests for the Vim state machine.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::editor::buffer::Position;

use super::*;

/// Feeds a literal key sequence and returns the last outcome.
fn run(state: &mut VimState, text: &str) -> VimOutcome {
    let mut outcome = VimOutcome::Pending;
    for key in keys(text) {
        outcome = state.feed(key);
    }
    outcome
}

fn once(text: &str) -> VimOutcome {
    let mut state = VimState::new();
    run(&mut state, text)
}

fn cmd(text: &str) -> VimCommand {
    match once(text) {
        VimOutcome::Command(command) => command,
        other => panic!("{text} produced {other:?}"),
    }
}

fn operate(operator: Operator, target: OperatorTarget, count: usize) -> VimAction {
    let _ = count;
    VimAction::Operate { operator, target }
}

#[test]
fn a_bare_motion_carries_its_count() {
    let got = cmd("3w");
    assert_eq!(got.count, 3);
    assert_eq!(got.action, VimAction::Motion(Motion::WordForward));
    assert_eq!(got.register, None);
}

#[test]
fn a_count_after_an_operator_applies_to_the_motion() {
    let got = cmd("d2w");
    assert_eq!(got.count, 2);
    assert_eq!(
        got.action,
        operate(
            Operator::Delete,
            OperatorTarget::Motion(Motion::WordForward),
            2
        )
    );
}

#[test]
fn counts_before_and_after_an_operator_multiply() {
    let got = cmd("2d3w");
    assert_eq!(got.count, 6);
    assert_eq!(
        got.action,
        operate(
            Operator::Delete,
            OperatorTarget::Motion(Motion::WordForward),
            6
        )
    );
}

#[test]
fn multi_digit_counts_accumulate() {
    assert_eq!(cmd("12j").count, 12);
    assert_eq!(cmd("2d10w").count, 20);
}

#[test]
fn a_leading_zero_is_the_line_start_motion() {
    assert_eq!(cmd("0").action, VimAction::Motion(Motion::LineStart));
    // Inside a count it is a digit.
    assert_eq!(cmd("10j").count, 10);
}

#[test]
fn doubling_an_operator_makes_it_linewise() {
    for (seq, operator) in [
        ("dd", Operator::Delete),
        ("yy", Operator::Yank),
        ("cc", Operator::Change),
        (">>", Operator::Indent),
        ("<<", Operator::Outdent),
    ] {
        let got = cmd(seq);
        assert_eq!(
            got.action,
            operate(operator, OperatorTarget::Line, 1),
            "{seq}"
        );
        assert_eq!(got.count, 1);
    }
    assert_eq!(cmd("3dd").count, 3);
}

#[test]
fn mismatched_operator_doubling_is_rejected() {
    assert_eq!(once("dy"), VimOutcome::Rejected);
}

#[test]
fn operators_take_text_objects() {
    assert_eq!(
        cmd("ciw").action,
        operate(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::inner(TextObjectKind::Word)),
            1
        )
    );
    assert_eq!(
        cmd("di\"").action,
        operate(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::inner(TextObjectKind::Quote('"'))),
            1
        )
    );
    assert_eq!(
        cmd("ya(").action,
        operate(
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::around(TextObjectKind::Paren)),
            1
        )
    );
    assert_eq!(
        cmd("da{").action,
        operate(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::around(TextObjectKind::Brace)),
            1
        )
    );
    assert_eq!(
        cmd("yip").action,
        operate(
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::inner(TextObjectKind::Paragraph)),
            1
        )
    );
}

#[test]
fn without_a_pending_operator_i_and_a_are_insert_commands() {
    // `i` and `a` only introduce a text object while an operator is pending.
    let mut state = VimState::new();
    assert_eq!(
        state.feed(VimKey::Char('i')),
        VimOutcome::Command(VimCommand::new(VimAction::Simple(
            SimpleCommand::InsertBefore
        )))
    );
    assert_eq!(
        state.feed(VimKey::Char('w')),
        VimOutcome::Command(VimCommand::new(VimAction::Motion(Motion::WordForward)))
    );
    assert_eq!(
        state.feed(VimKey::Char('a')),
        VimOutcome::Command(VimCommand::new(VimAction::Simple(
            SimpleCommand::InsertAfter
        )))
    );
}

#[test]
fn an_unknown_text_object_key_is_rejected() {
    assert_eq!(once("diz"), VimOutcome::Rejected);
}

#[test]
fn find_motions_take_their_target_character() {
    assert_eq!(
        cmd("fx").action,
        VimAction::Motion(Motion::FindForward('x'))
    );
    assert_eq!(
        cmd("Tz").action,
        VimAction::Motion(Motion::TillBackward('z'))
    );
    assert_eq!(
        cmd("2dt,").action,
        operate(
            Operator::Delete,
            OperatorTarget::Motion(Motion::TillForward(',')),
            2
        )
    );
}

#[test]
fn semicolon_and_comma_repeat_the_last_find() {
    let mut state = VimState::new();
    assert!(matches!(run(&mut state, "fx"), VimOutcome::Command(_)));
    match state.feed(VimKey::Char(';')) {
        VimOutcome::Command(c) => assert_eq!(c.action, VimAction::Motion(Motion::FindForward('x'))),
        other => panic!("{other:?}"),
    }
    match state.feed(VimKey::Char(',')) {
        VimOutcome::Command(c) => {
            assert_eq!(c.action, VimAction::Motion(Motion::FindBackward('x')));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn repeating_a_find_before_any_find_is_rejected() {
    assert_eq!(once(";"), VimOutcome::Rejected);
    assert_eq!(once(","), VimOutcome::Rejected);
}

#[test]
fn g_prefixed_commands_resolve() {
    assert_eq!(cmd("gg").action, VimAction::Motion(Motion::FileStart));
    assert_eq!(cmd("5gg").action, VimAction::Motion(Motion::GotoLine(5)));
    assert_eq!(cmd("G").action, VimAction::Motion(Motion::FileEnd));
    assert_eq!(cmd("7G").action, VimAction::Motion(Motion::GotoLine(7)));
    assert_eq!(
        cmd("guw").action,
        operate(
            Operator::Lowercase,
            OperatorTarget::Motion(Motion::WordForward),
            1
        )
    );
    assert_eq!(
        cmd("gUU").action,
        operate(Operator::Uppercase, OperatorTarget::Line, 1)
    );
    assert_eq!(
        cmd("g~~").action,
        operate(Operator::ToggleCase, OperatorTarget::Line, 1)
    );
    assert_eq!(
        cmd("dgg").action,
        operate(
            Operator::Delete,
            OperatorTarget::Motion(Motion::FileStart),
            1
        )
    );
    assert_eq!(once("gz"), VimOutcome::Rejected);
}

#[test]
fn registers_prefix_a_command() {
    let got = cmd("\"ayy");
    assert_eq!(got.register, Some('a'));
    assert_eq!(got.action, operate(Operator::Yank, OperatorTarget::Line, 1));

    let got = cmd("\"ap");
    assert_eq!(got.register, Some('a'));
    assert_eq!(got.action, VimAction::Simple(SimpleCommand::PasteAfter));
}

#[test]
fn a_bad_register_name_is_rejected() {
    assert_eq!(once("\"("), VimOutcome::Rejected);
}

#[test]
fn marks_set_and_jump() {
    assert_eq!(cmd("ma").action, VimAction::SetMark('a'));
    assert_eq!(
        cmd("`a").action,
        VimAction::GotoMark {
            mark: 'a',
            linewise: false
        }
    );
    assert_eq!(
        cmd("'a").action,
        VimAction::GotoMark {
            mark: 'a',
            linewise: true
        }
    );
    assert_eq!(once("m!"), VimOutcome::Rejected);
}

#[test]
fn mark_positions_are_remembered() {
    let mut state = VimState::new();
    match run(&mut state, "mq") {
        VimOutcome::Command(c) => assert_eq!(c.action, VimAction::SetMark('q')),
        other => panic!("{other:?}"),
    }
    state.set_mark('q', Position::new(4, 2));
    assert_eq!(state.mark('q'), Some(Position::new(4, 2)));
    assert_eq!(state.mark('z'), None);
}

#[test]
fn dot_replays_the_last_change() {
    let mut state = VimState::new();
    let first = match run(&mut state, "d2w") {
        VimOutcome::Command(c) => c,
        other => panic!("{other:?}"),
    };
    match state.feed(VimKey::Char('.')) {
        VimOutcome::Command(replay) => assert_eq!(replay, first),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dot_takes_a_new_count_when_one_is_given() {
    let mut state = VimState::new();
    assert!(matches!(run(&mut state, "dw"), VimOutcome::Command(_)));
    match run(&mut state, "3.") {
        VimOutcome::Command(replay) => {
            assert_eq!(replay.count, 3);
            assert_eq!(
                replay.action,
                operate(
                    Operator::Delete,
                    OperatorTarget::Motion(Motion::WordForward),
                    3
                )
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn dot_ignores_motions_and_yanks() {
    let mut state = VimState::new();
    assert!(matches!(run(&mut state, "x"), VimOutcome::Command(_)));
    assert!(matches!(run(&mut state, "yy"), VimOutcome::Command(_)));
    assert!(matches!(run(&mut state, "3w"), VimOutcome::Command(_)));
    match state.feed(VimKey::Char('.')) {
        VimOutcome::Command(replay) => {
            assert_eq!(replay.action, VimAction::Simple(SimpleCommand::DeleteChar));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn dot_before_any_change_is_rejected() {
    assert_eq!(once("."), VimOutcome::Rejected);
}

#[test]
fn a_substitute_parses_out_of_the_command_line() {
    let mut state = VimState::new();
    for key in keys(":%s/a/b/g") {
        assert_eq!(state.feed(key), VimOutcome::Pending);
    }
    match state.feed(VimKey::Enter) {
        VimOutcome::Command(c) => assert_eq!(
            c.action,
            VimAction::Substitute(Substitute {
                pattern: "a".to_string(),
                replacement: "b".to_string(),
                whole_file: true,
                all_occurrences: true,
                ignore_case: false,
            })
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_line_substitute_keeps_its_flags_apart() {
    let mut state = VimState::new();
    run(&mut state, ":s/x/y/i");
    match state.feed(VimKey::Enter) {
        VimOutcome::Command(c) => match c.action {
            VimAction::Substitute(sub) => {
                assert!(!sub.whole_file);
                assert!(!sub.all_occurrences);
                assert!(sub.ignore_case);
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn other_colon_commands_come_back_as_ex() {
    let mut state = VimState::new();
    run(&mut state, ":wq");
    match state.feed(VimKey::Enter) {
        VimOutcome::Command(c) => assert_eq!(c.action, VimAction::Ex("wq".to_string())),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_command_line_can_be_edited_and_cancelled() {
    let mut state = VimState::new();
    run(&mut state, ":wqz");
    assert_eq!(state.command_line(), "wqz");
    assert_eq!(state.feed(VimKey::Backspace), VimOutcome::Pending);
    assert_eq!(state.command_line(), "wq");
    assert_eq!(state.feed(VimKey::Escape), VimOutcome::Pending);
    assert_eq!(state.command_line(), "");
    assert!(!state.is_pending());
}

#[test]
fn an_empty_command_line_is_rejected() {
    let mut state = VimState::new();
    assert_eq!(state.feed(VimKey::Char(':')), VimOutcome::Pending);
    assert_eq!(state.feed(VimKey::Enter), VimOutcome::Rejected);
}

#[test]
fn simple_commands_resolve_with_counts() {
    assert_eq!(
        cmd("x").action,
        VimAction::Simple(SimpleCommand::DeleteChar)
    );
    assert_eq!(cmd("5x").count, 5);
    assert_eq!(
        cmd("rz").action,
        VimAction::Simple(SimpleCommand::ReplaceChar('z'))
    );
    assert_eq!(cmd("J").action, VimAction::Simple(SimpleCommand::JoinLines));
    assert_eq!(cmd("u").action, VimAction::Simple(SimpleCommand::Undo));

    let mut state = VimState::new();
    match state.feed(VimKey::Ctrl('r')) {
        VimOutcome::Command(c) => assert_eq!(c.action, VimAction::Simple(SimpleCommand::Redo)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn nonsense_sequences_are_rejected_and_reset_the_state() {
    let mut state = VimState::new();
    assert_eq!(run(&mut state, "dq"), VimOutcome::Rejected);
    assert!(!state.is_pending());

    assert_eq!(once("q"), VimOutcome::Rejected);
    assert_eq!(once("zz"), VimOutcome::Rejected);
    assert_eq!(once("d\\"), VimOutcome::Rejected);

    let mut state = VimState::new();
    assert_eq!(state.feed(VimKey::Tab), VimOutcome::Rejected);
}

#[test]
fn escape_cancels_a_partial_sequence() {
    let mut state = VimState::new();
    assert_eq!(state.feed(VimKey::Char('2')), VimOutcome::Pending);
    assert_eq!(state.feed(VimKey::Char('d')), VimOutcome::Pending);
    assert!(state.is_pending());
    assert_eq!(state.feed(VimKey::Escape), VimOutcome::Pending);
    assert!(!state.is_pending());
    // The next key starts fresh.
    assert_eq!(cmd("w").count, 1);
    assert_eq!(
        state.feed(VimKey::Char('w')),
        VimOutcome::Command(VimCommand::new(VimAction::Motion(Motion::WordForward)))
    );
}
