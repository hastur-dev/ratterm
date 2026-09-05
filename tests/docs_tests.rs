//! Tests that the documentation describes the code that exists.
//!
//! `docs/architecture.md` named three modules that were never there
//! (`ssh/hosts.rs`, `docker/remote.rs`, `extensions/`) and omitted seven that
//! were. Documentation drifts because nothing fails when it does, so these
//! tests fail instead.
//!
//! They check structure, not prose: that every module is named, that every
//! name refers to something real, and that a documented setting is one the
//! parser accepts. Wording is left to the writer.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The repository root, from this test file's location.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads a repository file, failing with the path if it is missing.
fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every `pub mod` declared at the top level of `src/lib.rs`.
fn declared_modules() -> BTreeSet<String> {
    read("src/lib.rs")
        .lines()
        .filter_map(|line| {
            // Only top-level declarations: nested ones are indented.
            let rest = line.strip_prefix("pub mod ")?;
            rest.strip_suffix(';').map(str::to_string)
        })
        .collect()
}

/// Every module named in the architecture document's module table.
///
/// Scoped to the "Module Structure" section: the document has other tables
/// with the same row shape, and a dependency is not a module.
fn documented_modules() -> BTreeSet<String> {
    let document = read("docs/architecture.md");
    let start = document
        .find("## Module Structure")
        .expect("docs/architecture.md has no Module Structure section");
    let rest = &document[start + "## Module Structure".len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());

    rest[..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let inner = trimmed.strip_prefix("| `")?;
            let (name, _) = inner.split_once("` |")?;
            // A module name, not a path or an expression.
            if name.contains('/') || name.contains('.') || name.contains(' ') {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

#[test]
fn every_module_is_documented() {
    let declared = declared_modules();
    let documented = documented_modules();

    assert!(
        !declared.is_empty(),
        "no modules were found in src/lib.rs; the parser is wrong, not the code"
    );

    let missing: Vec<&String> = declared.difference(&documented).collect();
    assert!(
        missing.is_empty(),
        "docs/architecture.md does not mention: {missing:?}"
    );
}

#[test]
fn every_documented_module_exists() {
    let declared = declared_modules();
    let documented = documented_modules();

    let invented: Vec<&String> = documented.difference(&declared).collect();
    assert!(
        invented.is_empty(),
        "docs/architecture.md names modules that do not exist: {invented:?}"
    );
}

#[test]
fn every_documented_module_has_a_source_file() {
    // A `pub mod` line proves the declaration; this proves the file.
    let root = repo_root().join("src");

    for module in declared_modules() {
        let as_file = root.join(format!("{module}.rs"));
        let as_directory = root.join(&module).join("mod.rs");
        assert!(
            as_file.exists() || as_directory.exists(),
            "module `{module}` has neither {} nor {}",
            as_file.display(),
            as_directory.display()
        );
    }
}

#[test]
fn the_archive_explains_what_it_holds() {
    // Archived planning documents are easy to mistake for open task lists.
    let readme = read("docs/archive/README.md");
    assert!(
        readme.contains("not instructions"),
        "the archive must say it is not a task list"
    );

    let archive = repo_root().join("docs/archive");
    for entry in std::fs::read_dir(&archive).expect("the archive directory") {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a file name");
        if name == "README.md" {
            continue;
        }
        assert!(
            readme.contains(name),
            "{name} is archived but not listed in docs/archive/README.md"
        );
    }
}

#[test]
fn no_diagnostic_output_is_committed_at_the_repository_root() {
    // Run output belongs in the temp directory; a committed one is a diff on
    // every run that says nothing about the code.
    let root = repo_root();
    assert!(
        !root.join("test_output.txt").exists(),
        "test_output.txt is a run artefact and should not be in the repository"
    );
}

#[test]
fn the_artefacts_a_run_leaves_behind_are_ignored() {
    // Some cannot be prevented — an MSYS tool crashing mid-run drops a
    // `.stackdump` wherever it was — so the check is that they cannot be
    // committed, not that they never appear.
    let ignore = read(".gitignore");
    for pattern in ["*.stackdump", "test-results/", "*.log"] {
        assert!(
            ignore.lines().any(|line| line.trim() == pattern),
            ".gitignore does not cover {pattern}"
        );
    }
}

#[test]
fn the_readme_and_the_manifest_agree_on_the_binaries() {
    let manifest = read("Cargo.toml");
    for binary in ["rat", "rat-agent"] {
        assert!(
            manifest.contains(&format!("name = \"{binary}\"")),
            "Cargo.toml does not build `{binary}`"
        );
    }

    let architecture = read("docs/architecture.md");
    assert!(
        architecture.contains("rat-agent"),
        "the architecture document does not mention the agent binary"
    );
}

/// Paths a document claims exist, written as `src/...` inside backticks.
fn referenced_paths(document: &str) -> BTreeSet<String> {
    let text = read(document);
    let mut found = BTreeSet::new();

    for chunk in text.split('`').skip(1).step_by(2) {
        let candidate = chunk.trim();
        if !candidate.starts_with("src/") {
            continue;
        }
        // Ignore anything with an expression in it rather than a plain path.
        if candidate.contains(' ') || candidate.contains("::") || candidate.contains('*') {
            continue;
        }
        found.insert(candidate.trim_end_matches(&['.', ','][..]).to_string());
    }

    found
}

#[test]
fn documents_do_not_reference_source_files_that_are_gone() {
    let root = repo_root();
    let mut broken: Vec<(String, String)> = Vec::new();

    for document in ["docs/architecture.md", "docs/automation.md", "README.md"] {
        for path in referenced_paths(document) {
            if !Path::new(&root).join(&path).exists() {
                broken.push((document.to_string(), path));
            }
        }
    }

    assert!(broken.is_empty(), "documents point at missing files: {broken:?}");
}
