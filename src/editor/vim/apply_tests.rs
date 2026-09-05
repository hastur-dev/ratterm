//! Tests that take a resolved Vim command and apply it to a buffer.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::editor::buffer::{Buffer, Position};

use super::*;

/// Feeds a literal key sequence and returns the last outcome.
fn run(state: &mut VimState, text: &str) -> VimOutcome {
    let mut outcome = VimOutcome::Pending;
    for key in keys(text) {
        outcome = state.feed(key);
    }
    outcome
}

fn cmd(text: &str) -> VimCommand {
    let mut state = VimState::new();
    match run(&mut state, text) {
        VimOutcome::Command(command) => command,
        other => panic!("{text} produced {other:?}"),
    }
}

#[test]
fn a_yank_and_a_put_through_a_named_register_round_trip() {
    let mut state = VimState::new();
    let mut registers = Registers::new();
    let buffer = Buffer::from_str("first line\nsecond line\n");

    let yank = match run(&mut state, "\"ayy") {
        VimOutcome::Command(c) => c,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        yank.action,
        VimAction::Operate {
            operator: Operator::Yank,
            target: OperatorTarget::Line
        }
    ));
    let text = buffer.line(0).unwrap_or_default();
    registers.set_yank(yank.register, RegisterContent::linewise(text));

    let put = match run(&mut state, "\"ap") {
        VimOutcome::Command(c) => c,
        other => panic!("{other:?}"),
    };
    let content = registers.get(put.register).expect("register a is filled");
    assert_eq!(content.text, "first line\n");
    assert!(content.linewise);
}

#[test]
fn a_resolved_delete_applies_to_a_buffer() {
    let mut buffer = Buffer::from_str("one two three four\n");
    let command = cmd("d2w");
    let VimAction::Operate {
        target: OperatorTarget::Motion(motion),
        ..
    } = command.action
    else {
        panic!("expected an operator over a motion");
    };
    let start = Position::new(0, 0);
    let (end, kind) =
        resolve_motion(&buffer, start, &motion, command.count).expect("motion resolves");
    assert_eq!(kind, MotionKind::Exclusive);
    buffer.delete_range(start, end);
    assert_eq!(buffer.text(), "three four\n");
}

#[test]
fn a_resolved_text_object_applies_to_a_buffer() {
    let mut buffer = Buffer::from_str("let s = \"hello\";\n");
    let command = cmd("di\"");
    let VimAction::Operate {
        target: OperatorTarget::TextObject(object),
        ..
    } = command.action
    else {
        panic!("expected an operator over a text object");
    };
    let (start, end) =
        resolve_text_object(&buffer, Position::new(0, 10), &object).expect("object resolves");
    buffer.delete_range(start, end);
    assert_eq!(buffer.text(), "let s = \"\";\n");
}
