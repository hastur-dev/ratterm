//! Vim normal-mode input, as an operator-pending state machine.
//!
//! [`VimState::feed`] takes one key at a time and answers with [`VimOutcome`]:
//! the sequence is incomplete, it resolved to a [`VimCommand`], or it is not a
//! command at all. Nothing here reads or writes a buffer, so the whole model can
//! be exercised without a terminal.
//!
//! Counts before and after an operator multiply, so `2d3w` arrives as a delete
//! with a count of six. Registers, marks, the last `f`/`t` search, and the last
//! change for `.` all live in the state.

pub mod command;
pub mod exec;
mod exec_simple;
pub mod keys;
pub mod motion;
pub mod registers;
pub mod state;
mod tables;
pub mod textobject;
mod visual;
mod walker;

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod tests;

pub use command::{
    Operator, OperatorTarget, SimpleCommand, Substitute, VimAction, VimCommand, parse_substitute,
};
pub use exec::{VimEffect, VimFeed, ex_effect};
pub use keys::{VimKey, keys};
pub use motion::{MAX_MOTION_COUNT, MAX_MOTION_STEPS, Motion, MotionKind, resolve_motion};
pub use registers::{RegisterContent, Registers};
pub use state::{VimOutcome, VimState};
pub use textobject::{TextObject, TextObjectKind, resolve_text_object};
