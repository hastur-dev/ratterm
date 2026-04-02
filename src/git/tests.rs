//! Tests for git API functions.
//!
//! Uses tempfile to create real git repositories for testing.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::api::*;

/// Creates a temporary git repo with an initial commit.
fn create_test_repo() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path();

    // git init
    run_git(path, &["init"]);
    run_git(path, &["config", "user.email", "test@test.com"]);
    run_git(path, &["config", "user.name", "Test User"]);

    // Create initial file and commit
    std::fs::write(path.join("hello.txt"), "Hello, world!\nLine 2\nLine 3\n")
        .expect("write file");
    run_git(path, &["add", "hello.txt"]);
    run_git(path, &["commit", "-m", "Initial commit"]);

    dir
}

/// Runs a git command in the given directory.
fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("git {:?} failed: {}", args, e));

    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

// ============================================================================
// Status tests
// ============================================================================

#[test]
fn test_git_status_clean_repo() {
    let dir = create_test_repo();
    let entries = git_status(dir.path()).expect("git_status");
    assert!(entries.is_empty(), "clean repo should have no status entries");
}

#[test]
fn test_git_status_modified_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "Modified content\n").expect("write");

    let entries = git_status(dir.path()).expect("git_status");
    assert!(!entries.is_empty(), "should have at least one entry");
    assert!(
        entries
            .iter()
            .any(|e| e.path == "hello.txt" && !e.staged && e.kind == StatusKind::Modified),
        "should show hello.txt as unstaged modified"
    );
}

#[test]
fn test_git_status_untracked_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("new_file.txt"), "New!\n").expect("write");

    let entries = git_status(dir.path()).expect("git_status");
    assert!(
        entries
            .iter()
            .any(|e| e.path == "new_file.txt" && !e.staged && e.kind == StatusKind::New),
        "should show new_file.txt as untracked"
    );
}

#[test]
fn test_git_status_staged_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "Staged change\n").expect("write");
    run_git(dir.path(), &["add", "hello.txt"]);

    let entries = git_status(dir.path()).expect("git_status");
    assert!(
        entries
            .iter()
            .any(|e| e.path == "hello.txt" && e.staged && e.kind == StatusKind::Modified),
        "should show hello.txt as staged modified"
    );
}

// ============================================================================
// Diff tests
// ============================================================================

#[test]
fn test_git_diff_modified_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "Changed content\nLine 2\nLine 3\n")
        .expect("write");

    let diff = git_diff(dir.path(), None).expect("git_diff");
    assert!(
        !diff.hunks.is_empty(),
        "modified file should produce diff hunks"
    );
    assert!(diff.additions > 0 || diff.deletions > 0, "should have changes");
}

#[test]
fn test_git_diff_clean_repo() {
    let dir = create_test_repo();
    let diff = git_diff(dir.path(), None).expect("git_diff");
    assert!(diff.hunks.is_empty(), "clean repo should have empty diff");
}

// ============================================================================
// Log tests
// ============================================================================

#[test]
fn test_git_log_returns_commits() {
    let dir = create_test_repo();
    let log = git_log(dir.path(), 10).expect("git_log");
    assert!(!log.is_empty(), "should have at least one commit");
    assert_eq!(log[0].author, "Test User");
    assert_eq!(log[0].message, "Initial commit");
    assert_eq!(log[0].short_hash.len(), 7);
}

#[test]
fn test_git_log_respects_limit() {
    let dir = create_test_repo();

    // Add more commits
    for i in 0..5 {
        let filename = format!("file{}.txt", i);
        std::fs::write(dir.path().join(&filename), format!("content {}", i)).expect("write");
        run_git(dir.path(), &["add", &filename]);
        run_git(dir.path(), &["commit", "-m", &format!("Commit {}", i)]);
    }

    let log = git_log(dir.path(), 3).expect("git_log");
    assert_eq!(log.len(), 3, "should respect limit");
}

// ============================================================================
// Blame tests
// ============================================================================

#[test]
fn test_git_blame_returns_lines() {
    let dir = create_test_repo();
    let file_path = dir.path().join("hello.txt");
    let blame = git_blame(dir.path(), &file_path).expect("git_blame");

    assert_eq!(blame.len(), 3, "should have 3 blame lines for 3-line file");
    assert_eq!(blame[0].author, "Test User");
    assert_eq!(blame[0].line_number, 1);
    assert_eq!(blame[0].content, "Hello, world!");
}

// ============================================================================
// Branch tests
// ============================================================================

#[test]
fn test_git_branch_list() {
    let dir = create_test_repo();
    let branches = git_branch_list(dir.path()).expect("git_branch_list");

    assert!(!branches.is_empty(), "should have at least one branch");
    assert!(
        branches.iter().any(|b| b.is_current),
        "should have a current branch"
    );
}

// ============================================================================
// Stash tests
// ============================================================================

#[test]
fn test_git_stash_list_empty() {
    let dir = create_test_repo();
    let stashes = git_stash_list(dir.path()).expect("git_stash_list");
    assert!(stashes.is_empty(), "new repo should have no stashes");
}

#[test]
fn test_git_stash_push_and_list() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "stash me\n").expect("write");
    run_git(dir.path(), &["add", "hello.txt"]);

    git_stash_op(dir.path(), StashOp::Push).expect("stash push");

    let stashes = git_stash_list(dir.path()).expect("stash list");
    assert_eq!(stashes.len(), 1, "should have one stash");
}

// ============================================================================
// Stage / Unstage tests
// ============================================================================

#[test]
fn test_git_stage_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "stage me\n").expect("write");

    let file_path = dir.path().join("hello.txt");
    git_stage_file(dir.path(), &file_path).expect("stage");

    let entries = git_status(dir.path()).expect("status");
    assert!(
        entries.iter().any(|e| e.path == "hello.txt" && e.staged),
        "file should be staged"
    );
}

#[test]
fn test_git_unstage_file() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "unstage me\n").expect("write");
    run_git(dir.path(), &["add", "hello.txt"]);

    let file_path = dir.path().join("hello.txt");
    git_unstage_file(dir.path(), &file_path).expect("unstage");

    let entries = git_status(dir.path()).expect("status");
    assert!(
        !entries.iter().any(|e| e.path == "hello.txt" && e.staged),
        "file should not be staged after unstage"
    );
}

// ============================================================================
// Commit tests
// ============================================================================

#[test]
fn test_git_commit_creates_commit() {
    let dir = create_test_repo();
    std::fs::write(dir.path().join("hello.txt"), "committed\n").expect("write");
    run_git(dir.path(), &["add", "hello.txt"]);

    git_commit(dir.path(), "Test commit message", false).expect("commit");

    let log = git_log(dir.path(), 1).expect("log");
    assert_eq!(log[0].message, "Test commit message");
}

// ============================================================================
// Conflict marker tests
// ============================================================================

#[test]
fn test_detect_conflict_markers_finds_markers() {
    let content = "normal line\n<<<<<<< HEAD\nour change\n=======\ntheir change\n>>>>>>> branch\nafter\n";
    let markers = detect_conflict_markers(content);
    assert_eq!(markers.len(), 3);
    assert!(markers.contains(&1)); // <<<<<<<
    assert!(markers.contains(&3)); // =======
    assert!(markers.contains(&5)); // >>>>>>>
}

#[test]
fn test_detect_conflict_markers_empty_content() {
    let markers = detect_conflict_markers("");
    assert!(markers.is_empty());
}

#[test]
fn test_detect_conflict_markers_no_markers() {
    let content = "normal line 1\nnormal line 2\n";
    let markers = detect_conflict_markers(content);
    assert!(markers.is_empty());
}

// ============================================================================
// Branch operations tests
// ============================================================================

#[test]
fn test_git_checkout_branch() {
    let dir = create_test_repo();

    // Create a new branch
    run_git(dir.path(), &["branch", "test-branch"]);

    // Checkout the new branch
    git_checkout_branch(dir.path(), "test-branch").expect("checkout");

    // Verify we're on the new branch
    let branch = git_current_branch(dir.path()).expect("current branch");
    assert_eq!(branch, "test-branch");
}

#[test]
fn test_git_current_branch() {
    let dir = create_test_repo();
    let branch = git_current_branch(dir.path()).expect("current branch");
    // Default branch could be 'main' or 'master' depending on git config
    assert!(
        !branch.is_empty(),
        "should return a branch name"
    );
}
