//! Pure API functions wrapping `git2` for git operations.
//!
//! All functions take a repo path and return `Result<T>` — no UI logic here.

use std::path::Path;

use git2::{DiffOptions, Repository, StatusOptions};

/// Error type for git operations.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// Underlying git2 error.
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    /// Path is not in a git repository.
    #[error("not a git repository: {0}")]
    NotARepo(String),
    /// File not found.
    #[error("file not found: {0}")]
    FileNotFound(String),
}

/// Result alias for git operations.
pub type GitResult<T> = Result<T, GitError>;

// ============================================================================
// Types
// ============================================================================

/// Kind of file status in the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// New file (untracked or added to index).
    New,
    /// Modified file.
    Modified,
    /// Deleted file.
    Deleted,
    /// Renamed file.
    Renamed,
    /// File has a merge conflict.
    Conflicted,
    /// Type changed (e.g., file to symlink).
    TypeChange,
}

/// A single file's status in the git repository.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    /// Relative file path.
    pub path: String,
    /// Whether the file is staged (in the index).
    pub staged: bool,
    /// The kind of change.
    pub kind: StatusKind,
}

/// A single line within a diff hunk.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Line origin: '+', '-', or ' ' (context).
    pub origin: char,
    /// The line content.
    pub content: String,
    /// Old line number (if applicable).
    pub old_lineno: Option<u32>,
    /// New line number (if applicable).
    pub new_lineno: Option<u32>,
}

/// A hunk within a diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Header line (e.g., `@@ -1,3 +1,4 @@`).
    pub header: String,
    /// Old start line.
    pub old_start: u32,
    /// New start line.
    pub new_start: u32,
    /// Lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// Result of a diff operation.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// File path this diff is for (if single-file diff).
    pub file_path: Option<String>,
    /// The hunks in the diff.
    pub hunks: Vec<DiffHunk>,
    /// Number of additions.
    pub additions: usize,
    /// Number of deletions.
    pub deletions: usize,
}

/// A single commit entry from git log.
#[derive(Debug, Clone)]
pub struct CommitEntry {
    /// Short hash (first 7 chars).
    pub short_hash: String,
    /// Full hash.
    pub hash: String,
    /// Commit message (first line).
    pub message: String,
    /// Author name.
    pub author: String,
    /// Author email.
    pub email: String,
    /// Timestamp (seconds since epoch).
    pub timestamp: i64,
}

/// A single line of blame output.
#[derive(Debug, Clone)]
pub struct BlameLine {
    /// Short commit hash.
    pub short_hash: String,
    /// Author name.
    pub author: String,
    /// Timestamp (seconds since epoch).
    pub timestamp: i64,
    /// Line number (1-based).
    pub line_number: usize,
    /// Line content.
    pub content: String,
}

/// A branch entry.
#[derive(Debug, Clone)]
pub struct BranchEntry {
    /// Branch name.
    pub name: String,
    /// Whether this is the current (HEAD) branch.
    pub is_current: bool,
    /// Whether this is a remote branch.
    pub is_remote: bool,
    /// Last commit hash on this branch (short).
    pub last_commit: String,
}

/// A stash entry.
#[derive(Debug, Clone)]
pub struct StashEntry {
    /// Stash index.
    pub index: usize,
    /// Stash message.
    pub message: String,
}

/// Stash operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashOp {
    /// Push current changes to stash.
    Push,
    /// Pop the top stash entry.
    Pop,
    /// Drop a specific stash entry by index.
    Drop(usize),
}

// ============================================================================
// API Functions
// ============================================================================

/// Opens the git repository at or above `repo_path`.
fn open_repo(repo_path: &Path) -> GitResult<Repository> {
    Repository::discover(repo_path).map_err(|_| GitError::NotARepo(repo_path.display().to_string()))
}

/// Returns the status of all files in the working tree and index.
pub fn git_status(repo_path: &Path) -> GitResult<Vec<StatusEntry>> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut entries = Vec::with_capacity(statuses.len());

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let status = entry.status();

        // Index (staged) changes
        if status.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
        ) {
            let kind = if status.contains(git2::Status::INDEX_NEW) {
                StatusKind::New
            } else if status.contains(git2::Status::INDEX_MODIFIED) {
                StatusKind::Modified
            } else if status.contains(git2::Status::INDEX_DELETED) {
                StatusKind::Deleted
            } else if status.contains(git2::Status::INDEX_RENAMED) {
                StatusKind::Renamed
            } else {
                StatusKind::TypeChange
            };
            entries.push(StatusEntry {
                path: path.clone(),
                staged: true,
                kind,
            });
        }

        // Working tree (unstaged) changes
        if status.intersects(
            git2::Status::WT_NEW
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE,
        ) {
            let kind = if status.contains(git2::Status::WT_NEW) {
                StatusKind::New
            } else if status.contains(git2::Status::WT_MODIFIED) {
                StatusKind::Modified
            } else if status.contains(git2::Status::WT_DELETED) {
                StatusKind::Deleted
            } else if status.contains(git2::Status::WT_RENAMED) {
                StatusKind::Renamed
            } else {
                StatusKind::TypeChange
            };
            entries.push(StatusEntry {
                path: path.clone(),
                staged: false,
                kind,
            });
        }

        // Conflicted
        if status.contains(git2::Status::CONFLICTED) {
            entries.push(StatusEntry {
                path,
                staged: false,
                kind: StatusKind::Conflicted,
            });
        }
    }

    assert!(entries.len() <= statuses.len() * 2, "entry count sanity check");
    Ok(entries)
}

/// Collects diff data from a `git2::Diff` using `print` (single-callback API).
fn collect_diff(diff: &git2::Diff, file_path: Option<String>) -> GitResult<DiffResult> {
    let mut result = DiffResult {
        file_path,
        hunks: Vec::new(),
        additions: 0,
        deletions: 0,
    };

    diff.print(git2::DiffFormat::Patch, |_delta, hunk, line| {
        let origin = line.origin();

        // When we see a hunk header line, start a new hunk
        if origin == 'H' {
            if let Some(h) = hunk {
                let header = String::from_utf8_lossy(h.header()).to_string();
                result.hunks.push(DiffHunk {
                    header,
                    old_start: h.old_start(),
                    new_start: h.new_start(),
                    lines: Vec::new(),
                });
            }
            return true;
        }

        // Skip file header lines
        if origin == 'F' {
            return true;
        }

        match origin {
            '+' => result.additions += 1,
            '-' => result.deletions += 1,
            _ => {}
        }

        if let Some(last_hunk) = result.hunks.last_mut() {
            let content = String::from_utf8_lossy(line.content()).to_string();
            last_hunk.lines.push(DiffLine {
                origin,
                content,
                old_lineno: line.old_lineno(),
                new_lineno: line.new_lineno(),
            });
        }

        true
    })?;

    Ok(result)
}

/// Returns the diff for the working tree (or a specific file).
pub fn git_diff(repo_path: &Path, file: Option<&Path>) -> GitResult<DiffResult> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let mut opts = DiffOptions::new();

    if let Some(f) = file {
        let relative = f
            .strip_prefix(repo.workdir().unwrap_or(repo_path))
            .unwrap_or(f);
        opts.pathspec(relative);
    }

    let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    collect_diff(&diff, file.map(|f| f.display().to_string()))
}

/// Returns the staged diff (index vs HEAD).
pub fn git_diff_staged(repo_path: &Path) -> GitResult<DiffResult> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let head_tree = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok());

    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
    collect_diff(&diff, None)
}

/// Returns the git log (most recent commits).
pub fn git_log(repo_path: &Path, limit: usize) -> GitResult<Vec<CommitEntry>> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");
    assert!(limit > 0, "limit must be positive");
    assert!(limit <= 10_000, "limit must be reasonable");

    let repo = open_repo(repo_path)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut entries = Vec::with_capacity(limit.min(256));

    for (i, oid_result) in revwalk.enumerate() {
        if i >= limit {
            break;
        }
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let hash = oid.to_string();
        let short_hash = hash[..7.min(hash.len())].to_string();
        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let author = commit.author();

        entries.push(CommitEntry {
            short_hash,
            hash,
            message,
            author: author.name().unwrap_or("").to_string(),
            email: author.email().unwrap_or("").to_string(),
            timestamp: author.when().seconds(),
        });
    }

    Ok(entries)
}

/// Returns blame information for a file.
pub fn git_blame(repo_path: &Path, file: &Path) -> GitResult<Vec<BlameLine>> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let workdir = repo.workdir().unwrap_or(repo_path);
    let relative = make_relative(workdir, file);

    let blame = repo.blame_file(&relative, None)?;
    let mut lines = Vec::new();

    // Read the file content for line text
    let abs_path = workdir.join(relative);
    let content = std::fs::read_to_string(&abs_path)
        .map_err(|_| GitError::FileNotFound(abs_path.display().to_string()))?;

    for (line_idx, line_text) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        if let Some(hunk) = blame.get_line(line_num) {
            let oid = hunk.final_commit_id();
            let hash = oid.to_string();
            let short_hash = hash[..7.min(hash.len())].to_string();

            let sig = hunk.final_signature();
            let author = sig.name().unwrap_or("").to_string();
            let timestamp = sig.when().seconds();

            lines.push(BlameLine {
                short_hash,
                author,
                timestamp,
                line_number: line_num,
                content: line_text.to_string(),
            });
        }
    }

    Ok(lines)
}

/// Returns the list of branches.
pub fn git_branch_list(repo_path: &Path) -> GitResult<Vec<BranchEntry>> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let branches = repo.branches(None)?;
    let head_ref = repo.head().ok();
    let head_name = head_ref
        .as_ref()
        .and_then(|h| h.shorthand().map(String::from));

    let mut entries = Vec::new();

    for branch_result in branches {
        let (branch, branch_type) = branch_result?;
        let name = branch.name()?.unwrap_or("").to_string();
        let is_remote = branch_type == git2::BranchType::Remote;
        let is_current = head_name.as_deref() == Some(&name) && !is_remote;

        let last_commit = branch
            .get()
            .peel_to_commit()
            .map(|c| c.id().to_string()[..7].to_string())
            .unwrap_or_default();

        entries.push(BranchEntry {
            name,
            is_current,
            is_remote,
            last_commit,
        });
    }

    Ok(entries)
}

/// Returns the stash list.
pub fn git_stash_list(repo_path: &Path) -> GitResult<Vec<StashEntry>> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let mut entries = Vec::new();

    // git2 requires mutable repo for stash_foreach
    let mut repo = repo;
    repo.stash_foreach(|index, message, _oid| {
        entries.push(StashEntry {
            index,
            message: message.to_string(),
        });
        true
    })?;

    Ok(entries)
}

/// Creates a commit with the given message.
pub fn git_commit(repo_path: &Path, message: &str, amend: bool) -> GitResult<()> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");
    assert!(!message.is_empty(), "commit message must not be empty");

    let repo = open_repo(repo_path)?;
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    if amend {
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        commit.amend(Some("HEAD"), None, None, None, Some(message), Some(&tree))?;
    } else {
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
    }

    Ok(())
}

/// Converts a file path to be relative to the repo workdir.
///
/// Handles Windows short paths (8.3 names) by canonicalizing both paths.
fn make_relative(workdir: &Path, file: &Path) -> std::path::PathBuf {
    // Try simple strip_prefix first
    if let Ok(rel) = file.strip_prefix(workdir) {
        return rel.to_path_buf();
    }

    // On Windows, paths may differ (short vs long names). Canonicalize both.
    let canon_workdir = std::fs::canonicalize(workdir).unwrap_or_else(|_| workdir.to_path_buf());
    let canon_file = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());

    canon_file
        .strip_prefix(&canon_workdir)
        .map(|r| r.to_path_buf())
        .unwrap_or_else(|_| file.to_path_buf())
}

/// Stages a file (adds to index).
pub fn git_stage_file(repo_path: &Path, file: &Path) -> GitResult<()> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let workdir = repo.workdir().unwrap_or(repo_path);
    let relative = make_relative(workdir, file);

    let mut index = repo.index()?;
    index.add_path(&relative)?;
    index.write()?;

    Ok(())
}

/// Unstages a file (removes from index, keeping working tree).
pub fn git_unstage_file(repo_path: &Path, file: &Path) -> GitResult<()> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let workdir = repo.workdir().unwrap_or(repo_path);
    let relative = make_relative(workdir, file);

    let head = repo.head()?.peel_to_commit()?;
    let head_tree = head.tree()?;

    repo.reset_default(Some(head.as_object()), [&relative])?;

    // If file didn't exist in HEAD, remove it from the index entirely
    if head_tree.get_path(&relative).is_err() {
        let mut index = repo.index()?;
        let _ = index.remove_path(&relative);
        index.write()?;
    }

    Ok(())
}

/// Checks out a branch.
pub fn git_checkout_branch(repo_path: &Path, branch: &str) -> GitResult<()> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");
    assert!(!branch.is_empty(), "branch name must not be empty");

    let repo = open_repo(repo_path)?;
    let (object, reference) = repo.revparse_ext(branch)?;

    repo.checkout_tree(&object, None)?;

    if let Some(reference) = reference {
        let refname = reference.name().unwrap_or("");
        repo.set_head(refname)?;
    } else {
        repo.set_head_detached(object.id())?;
    }

    Ok(())
}

/// Performs a stash operation.
pub fn git_stash_op(repo_path: &Path, op: StashOp) -> GitResult<()> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let mut repo = open_repo(repo_path)?;

    match op {
        StashOp::Push => {
            let sig = repo.signature()?;
            repo.stash_save(&sig, "Stash from ratterm", None)?;
        }
        StashOp::Pop => {
            repo.stash_pop(0, None)?;
        }
        StashOp::Drop(index) => {
            repo.stash_drop(index)?;
        }
    }

    Ok(())
}

/// Detects conflict markers in file content.
///
/// Returns line numbers (0-based) that contain conflict markers
/// (`<<<<<<<`, `=======`, `>>>>>>>`).
pub fn detect_conflict_markers(content: &str) -> Vec<usize> {
    assert!(!content.is_empty() || content.is_empty(), "content validation");

    let mut markers = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<<<<<<<")
            || trimmed.starts_with("=======")
            || trimmed.starts_with(">>>>>>>")
        {
            markers.push(i);
        }
    }
    markers
}

/// Returns the current branch name (or HEAD hash if detached).
pub fn git_current_branch(repo_path: &Path) -> GitResult<String> {
    assert!(!repo_path.as_os_str().is_empty(), "repo_path must not be empty");

    let repo = open_repo(repo_path)?;
    let head = repo.head()?;

    if head.is_branch() {
        Ok(head.shorthand().unwrap_or("HEAD").to_string())
    } else {
        let oid = head.target().unwrap_or(git2::Oid::zero());
        Ok(format!("({})", &oid.to_string()[..7]))
    }
}
