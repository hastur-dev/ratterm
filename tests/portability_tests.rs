//! Tests that the tree stays buildable on every platform it claims to support.
//!
//! These exist because of a specific failure: `src/ssh/collector.rs` carried
//! `#[cfg(windows)]` twice on `use tracing::{debug, error, info, warn};`, while
//! the four macros were called unconditionally. On Windows both attributes were
//! true and the file compiled, so a local Windows gate passed and the change was
//! pushed. Every non-Windows job then failed at its first compile step — clippy,
//! three test jobs, two scenario jobs, two headless-smoke jobs, the docs build
//! and the MSRV check, ten jobs from one line.
//!
//! A unit test cannot catch a compile error on a platform this host is not, so
//! these read the sources and the workflow instead. They check shape, not
//! behaviour: that a platform gate is written the way it was meant to be, and
//! that CI still compiles the crate somewhere other than Windows.

#![allow(clippy::expect_used, clippy::unwrap_used)]

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

/// Every `.rs` file under `src/` and `tests/`, as (repository-relative path,
/// contents).
fn rust_sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut found = Vec::new();
    for top in ["src", "tests"] {
        collect(&root.join(top), &root, &mut found);
    }
    assert!(
        found.len() > 100,
        "only {} sources found; the walk is looking in the wrong place",
        found.len()
    );
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Recursively collects `.rs` files below `dir`.
fn collect(dir: &Path, root: &Path, found: &mut Vec<(String, String)>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect(&path, root, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let relative = path
                .strip_prefix(root)
                .expect("path below the repository root")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{relative}: {e}"));
            found.push((relative, text));
        }
    }
}

/// True for an attribute line that gates an item on the target platform.
fn is_cfg_attribute(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("#[cfg(") && !line.starts_with("#[cfg_attr(")
}

/// True for a `#[cfg(...)]` that selects on the platform rather than on, say,
/// `test` or a feature.
fn is_platform_gate(line: &str) -> bool {
    let line = line.trim();
    is_cfg_attribute(line)
        && (line.contains("windows")
            || line.contains("unix")
            || line.contains("target_os")
            || line.contains("target_family")
            || line.contains("target_arch"))
}

/// Two `#[cfg(...)]` attributes on one item mean "both must hold", which is
/// almost never what was meant and is invisible on the platform where both are
/// true. `#[cfg(all(..))]` says it once and says it out loud.
#[test]
fn stacked_cfg_attributes_are_never_written() {
    let mut offenders = Vec::new();

    for (path, text) in rust_sources() {
        let lines: Vec<&str> = text.lines().collect();
        for pair in lines.windows(2).enumerate() {
            let (index, window) = pair;
            if is_cfg_attribute(window[0]) && is_cfg_attribute(window[1]) {
                offenders.push(format!(
                    "{}:{}: {} followed by {}",
                    path,
                    index + 1,
                    window[0].trim(),
                    window[1].trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "stacked cfg attributes; write one #[cfg(all(..))] instead:\n{}",
        offenders.join("\n")
    );
}

/// The tracing macros are called from unconditional code all over this crate,
/// so the import that brings them in must be unconditional too. Gating it on a
/// platform compiles there and nowhere else.
#[test]
fn tracing_imports_are_not_platform_gated() {
    let mut offenders = Vec::new();

    for (path, text) in rust_sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !line.trim_start().starts_with("use tracing::") {
                continue;
            }
            let Some(previous) = index.checked_sub(1).map(|i| lines[i]) else {
                continue;
            };
            if is_platform_gate(previous) {
                offenders.push(format!("{}:{}: {}", path, index + 1, previous.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "platform-gated tracing imports:\n{}",
        offenders.join("\n")
    );
}

/// The matrix that would have caught the failure above on the first push. A
/// platform removed from it is a platform nothing compiles the crate for.
#[test]
fn ci_runs_the_suite_on_every_supported_platform() {
    let workflow = read(".github/workflows/ci.yml");

    for runner in [
        "ubuntu-latest",
        "windows-latest",
        "macos-latest",
        "ubuntu-24.04-arm",
    ] {
        assert!(
            workflow.contains(runner),
            "the CI matrix no longer includes {runner}"
        );
    }

    for job in [
        "cargo clippy --all-targets --all-features -- -D warnings",
        "cargo test --all-features --verbose",
        "cargo test --doc",
        "cargo build --release",
        "cargo doc --no-deps --all-features",
        "cargo check --all-features",
    ] {
        assert!(workflow.contains(job), "CI no longer runs `{job}`");
    }
}

/// Every gate in this workflow turns a warning into a failure. Losing one is
/// how a lint stops being enforced without anyone deciding that it should.
#[test]
fn ci_keeps_warnings_fatal() {
    let workflow = read(".github/workflows/ci.yml");

    assert!(
        workflow.contains(r#"RUSTFLAGS: "-D warnings""#),
        "RUSTFLAGS no longer denies warnings"
    );
    assert!(
        workflow.contains(r#"RUSTDOCFLAGS: "-D warnings""#),
        "RUSTDOCFLAGS no longer denies warnings"
    );
    assert!(
        workflow.contains("-- -D warnings"),
        "clippy no longer denies warnings"
    );
}

/// The manifest's MSRV and the toolchain the MSRV job installs have to be the
/// same number, or the job checks a version nobody promised.
#[test]
fn the_msrv_job_pins_the_manifest_version() {
    let manifest = read("Cargo.toml");
    let workflow = read(".github/workflows/ci.yml");

    let declared = manifest
        .lines()
        .find_map(|line| line.strip_prefix("rust-version = "))
        .expect("rust-version in Cargo.toml")
        .trim()
        .trim_matches('"')
        .to_string();

    assert!(
        workflow.contains(&format!(r#"toolchain: "{declared}""#)),
        "the MSRV job does not install {declared}"
    );
}
