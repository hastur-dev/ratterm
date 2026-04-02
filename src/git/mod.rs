//! Git integration module.
//!
//! Provides pure API functions wrapping `git2` for status, diff, log,
//! blame, branch, stash, staging, and commit operations.

pub mod api;
pub mod dashboard;
pub mod gutter;

#[cfg(test)]
mod tests;

pub use api::{
    BlameLine, BranchEntry, CommitEntry, DiffHunk, DiffLine, DiffResult, StashEntry, StashOp,
    StatusEntry, StatusKind,
};
pub use dashboard::{GitDashboard, GitDashboardMode, GitDashboardView};
pub use gutter::{GutterMark, compute_gutter_indicators};
