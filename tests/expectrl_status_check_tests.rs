#![cfg(windows)]
//! Local SSH & Health status verification tests.
//!
//! These diagnostic tests load saved SSH hosts from `~/.ratterm/ssh_hosts.toml`
//! and verify their connectivity and health status using the API layer directly
//! (no TUI/ConPTY). They print a comprehensive report of per-host statuses
//! and flag any errors (offline, unreachable, auth failures).
//!
//! ## Prerequisites
//! - Saved SSH hosts with credentials in `~/.ratterm/ssh_hosts.toml`
//!
//! ## Run
//! ```bash
//! cargo test --test expectrl_status_check_tests -- --ignored --nocapture
//! ```
//!
//! Individual tests:
//! ```bash
//! cargo test --test expectrl_status_check_tests test_ssh_reachability -- --ignored --nocapture
//! cargo test --test expectrl_status_check_tests test_health_metrics -- --ignored --nocapture
//! cargo test --test expectrl_status_check_tests test_full_status_diagnostic -- --ignored --nocapture
//! ```

#![allow(clippy::expect_used)]

use std::time::Duration;

/// Seconds to wait for health metrics collection to finish.
const COLLECTION_WAIT_SECS: u64 = 15;

/// SSH Manager status values (Title Case, from `ConnectionStatus::as_str()`).
const SSH_STATUSES: &[&str] = &["Connected", "Reachable", "Unreachable", "Unknown"];

/// Health Dashboard status values (UPPERCASE, from `MetricStatus::as_str()`).
const HEALTH_STATUSES: &[&str] = &["ONLINE", "OFFLINE", "COLLECTING", "ERROR", "UNKNOWN"];

/// Characters after a hostname to search for a nearby status keyword.
#[allow(dead_code)]
const STATUS_SEARCH_WINDOW: usize = 200;

// ============================================================================
// Types
// ============================================================================

/// A host loaded from `~/.ratterm/ssh_hosts.toml` with resolved data.
struct HostEntry {
    id: u32,
    hostname: String,
    port: u16,
    display: String,
    has_credentials: bool,
}

/// Per-host diagnostic result.
struct DiagResult {
    display: String,
    hostname: String,
    reachable: bool,
    health_status: Option<String>,
    error: Option<String>,
}

// ============================================================================
// Pure helper functions (also used by unit tests)
// ============================================================================

/// Searches for a status keyword near an anchor string in screen text.
///
/// Finds **every** occurrence of `anchor` in `screen`, checks whether
/// any of `candidates` appears within `window` characters after it,
/// and returns the **last** match.
fn find_status_near(
    screen: &str,
    anchor: &str,
    candidates: &[&str],
    window: usize,
) -> Option<String> {
    assert!(!anchor.is_empty(), "anchor must not be empty");
    assert!(!candidates.is_empty(), "candidates must not be empty");

    let mut last_match: Option<String> = None;
    let mut pos = 0;

    while pos < screen.len() {
        let remaining = &screen[pos..];
        let Some(offset) = remaining.find(anchor) else {
            break;
        };

        let abs = pos + offset;
        let end = (abs + anchor.len() + window).min(screen.len());
        let region = &screen[abs..end];

        for &cand in candidates {
            if region.contains(cand) {
                last_match = Some(cand.to_string());
                break;
            }
        }

        pos = abs + anchor.len();
    }

    last_match
}

/// Loads saved SSH hosts from `~/.ratterm/ssh_hosts.toml`.
///
/// Returns a tuple of `(host_list, entries)` where `entries` is a Vec
/// of resolved `HostEntry` structs. Returns `None` if loading fails.
fn load_saved_hosts() -> Option<(ratterm::SSHHostList, Vec<HostEntry>)> {
    let mut storage = ratterm::SSHStorage::new();
    let host_list = match storage.load() {
        Ok(h) => h,
        Err(e) => {
            println!("  WARNING: Cannot load SSH hosts: {}", e);
            println!("  Path: {:?}", ratterm::SSHStorage::default_path());
            return None;
        }
    };

    let entries: Vec<HostEntry> = host_list
        .hosts()
        .map(|h| HostEntry {
            id: h.id,
            hostname: h.hostname.clone(),
            port: h.port,
            display: h.display().to_string(),
            has_credentials: host_list.get_credentials(h.id).is_some(),
        })
        .collect();

    Some((host_list, entries))
}

/// Prints diagnostic results and returns a list of error messages.
fn print_diag_results(results: &[DiagResult]) -> Vec<String> {
    let mut errors = Vec::new();

    for r in results {
        let label = if r.display == r.hostname {
            r.hostname.clone()
        } else {
            format!("{} ({})", r.display, r.hostname)
        };

        match (&r.health_status, r.reachable, &r.error) {
            (Some(status), _, _) if status == "ONLINE" => {
                println!("  [OK] {}: ONLINE", label);
            }
            (Some(status), _, _) if status == "COLLECTING" => {
                println!("  [..] {}: COLLECTING (still in progress)", label);
            }
            (_, _, Some(err)) => {
                println!("  [!!] {}: {}", label, err);
                errors.push(format!("{}: {}", label, err));
            }
            (Some(status), _, _) => {
                println!("  [!!] {}: {}", label, status);
                errors.push(format!("{}: {}", label, status));
            }
            (None, true, None) => {
                println!("  [OK] {}: Reachable (port 22 open)", label);
            }
            (None, false, None) => {
                println!("  [!!] {}: Unreachable", label);
                errors.push(format!("{}: Unreachable", label));
            }
        }
    }

    errors
}

// ============================================================================
// Unit tests for find_status_near
// ============================================================================

#[test]
fn test_find_status_near_found() {
    let screen = "row 1  192.168.1.10   Connected  more text";
    let result = find_status_near(screen, "192.168.1.10", SSH_STATUSES, 50);
    assert_eq!(result, Some("Connected".to_string()));
}

#[test]
fn test_find_status_near_not_found() {
    let screen = "row 1  192.168.1.10   blah  more text";
    let result = find_status_near(screen, "192.168.1.10", SSH_STATUSES, 50);
    assert!(result.is_none());
}

#[test]
fn test_find_status_near_anchor_missing() {
    let screen = "no matching hostname anywhere";
    let result = find_status_near(screen, "192.168.1.10", SSH_STATUSES, 50);
    assert!(result.is_none());
}

#[test]
fn test_find_status_near_window_limit() {
    let screen = "192.168.1.10 some padding  Connected";
    let short = find_status_near(screen, "192.168.1.10", SSH_STATUSES, 10);
    assert!(short.is_none(), "window=10 should not reach Connected");

    let long = find_status_near(screen, "192.168.1.10", SSH_STATUSES, 30);
    assert_eq!(long, Some("Connected".to_string()));
}

#[test]
fn test_find_status_near_health_status() {
    let screen = "ubuntu-server (10.0.0.18) [ONLINE]  CPU: 45%";
    let result = find_status_near(screen, "10.0.0.18", HEALTH_STATUSES, 50);
    assert_eq!(result, Some("ONLINE".to_string()));
}

#[test]
fn test_find_status_near_multiple_occurrences() {
    let screen = "10.0.0.18 [OFFLINE] ... 10.0.0.18  Connected";
    let result = find_status_near(screen, "10.0.0.18", SSH_STATUSES, 50);
    assert_eq!(result, Some("Connected".to_string()));
}

#[test]
fn test_find_status_near_empty_screen() {
    let result = find_status_near("", "10.0.0.18", SSH_STATUSES, 50);
    assert!(result.is_none());
}

#[test]
fn test_find_status_near_health_offline() {
    let screen = "Ai Rock5c (10.0.0.19) [OFFLINE]";
    let result = find_status_near(screen, "10.0.0.19", HEALTH_STATUSES, 50);
    assert_eq!(result, Some("OFFLINE".to_string()));
}

#[test]
fn test_find_status_near_case_sensitive() {
    let screen = "10.0.0.18 Unknown";
    let health = find_status_near(screen, "10.0.0.18", HEALTH_STATUSES, 50);
    assert!(
        health.is_none(),
        "title-case Unknown must not match UNKNOWN"
    );

    let ssh = find_status_near(screen, "10.0.0.18", SSH_STATUSES, 50);
    assert_eq!(ssh, Some("Unknown".to_string()));
}

#[test]
fn test_find_status_near_returns_last_match() {
    let screen = "row 1  10.0.0.18  Unknown  ...padding...\nrow 2  10.0.0.18  Reachable";
    let result = find_status_near(screen, "10.0.0.18", SSH_STATUSES, 50);
    assert_eq!(result, Some("Reachable".to_string()));
}

#[test]
fn test_print_diag_results_ok() {
    let results = vec![DiagResult {
        display: "server-1".into(),
        hostname: "10.0.0.1".into(),
        reachable: true,
        health_status: Some("ONLINE".into()),
        error: None,
    }];
    let errors = print_diag_results(&results);
    assert!(errors.is_empty());
}

#[test]
fn test_print_diag_results_error() {
    let results = vec![DiagResult {
        display: "server-1".into(),
        hostname: "10.0.0.1".into(),
        reachable: false,
        health_status: None,
        error: Some("Connection refused".into()),
    }];
    let errors = print_diag_results(&results);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Connection refused"));
}

#[test]
fn test_print_diag_results_reachable_no_health() {
    let results = vec![DiagResult {
        display: "10.0.0.5".into(),
        hostname: "10.0.0.5".into(),
        reachable: true,
        health_status: None,
        error: None,
    }];
    let errors = print_diag_results(&results);
    assert!(errors.is_empty());
}

#[test]
fn test_print_diag_results_unreachable() {
    let results = vec![DiagResult {
        display: "ghost".into(),
        hostname: "10.0.0.99".into(),
        reachable: false,
        health_status: None,
        error: None,
    }];
    let errors = print_diag_results(&results);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Unreachable"));
}

// ============================================================================
// Diagnostic: SSH reachability check (TCP port 22)
// ============================================================================

#[test]
#[ignore] // Requires network access and saved hosts
fn test_ssh_reachability() {
    println!("\n{}", "=".repeat(60));
    println!("  SSH Reachability Check (TCP port 22)");
    println!("{}\n", "=".repeat(60));

    let Some((_host_list, entries)) = load_saved_hosts() else {
        println!("SKIP: Cannot load SSH hosts.");
        return;
    };

    if entries.is_empty() {
        println!("SKIP: No saved SSH hosts found.");
        println!("  Add hosts to {:?}", ratterm::SSHStorage::default_path());
        return;
    }

    println!("Checking {} hosts...\n", entries.len());

    let mut results = Vec::with_capacity(entries.len());

    for entry in &entries {
        let reachable = ratterm::NetworkScanner::check_host(&entry.hostname, entry.port);
        results.push(DiagResult {
            display: entry.display.clone(),
            hostname: entry.hostname.clone(),
            reachable,
            health_status: None,
            error: None,
        });
    }

    let errors = print_diag_results(&results);
    let reachable = results.iter().filter(|r| r.reachable).count();

    println!("\n  Summary: {}/{} reachable", reachable, entries.len());

    if !errors.is_empty() {
        println!("\n  Errors ({}):", errors.len());
        for e in &errors {
            println!("    ! {}", e);
        }
    }

    println!("\n{}\n", "=".repeat(60));
}

// ============================================================================
// Diagnostic: Health metrics collection via SSH
// ============================================================================

#[test]
#[ignore] // Requires network access and saved hosts with credentials
fn test_health_metrics() {
    println!("\n{}", "=".repeat(60));
    println!("  Health Metrics Collection");
    println!("{}\n", "=".repeat(60));

    let Some((host_list, entries)) = load_saved_hosts() else {
        println!("SKIP: Cannot load SSH hosts.");
        return;
    };

    let credentialed: Vec<&HostEntry> = entries.iter().filter(|e| e.has_credentials).collect();

    if credentialed.is_empty() {
        println!("SKIP: No hosts with credentials found.");
        println!("  Health metrics require saved credentials.");
        return;
    }

    println!(
        "Loaded {} hosts ({} with credentials)\n",
        entries.len(),
        credentialed.len()
    );

    // Build collection info for credentialed hosts.
    let collection_info = ratterm::build_collection_info(&host_list);
    assert!(
        !collection_info.is_empty(),
        "build_collection_info should return non-empty for hosts with credentials"
    );

    // Collect metrics.
    let mut collector = ratterm::MetricsCollector::new();
    collector.collect(&collection_info);

    println!(
        "  Collecting metrics ({}s timeout)...",
        COLLECTION_WAIT_SECS
    );

    // Poll until complete or timeout.
    let deadline = std::time::Instant::now() + Duration::from_secs(COLLECTION_WAIT_SECS);
    while !collector.is_collection_complete() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        collector.poll_results();
    }
    // Final poll to pick up any remaining results.
    collector.poll_results();
    println!("  Collection finished.\n");

    // Build results.
    let mut results = Vec::with_capacity(credentialed.len());

    for entry in &credentialed {
        let metrics = collector.get_metrics(entry.id);
        let (health_status, error) = match metrics {
            Some(m) => {
                let status = m.status.as_str().to_string();
                let err = m.error.clone();
                (Some(status), err)
            }
            None => (None, Some("No metrics collected".into())),
        };

        results.push(DiagResult {
            display: entry.display.clone(),
            hostname: entry.hostname.clone(),
            reachable: health_status.as_deref() == Some("ONLINE"),
            health_status,
            error,
        });
    }

    let errors = print_diag_results(&results);

    // Report skipped hosts.
    for entry in entries.iter().filter(|e| !e.has_credentials) {
        println!(
            "  [--] {} ({}): skipped (no credentials)",
            entry.display, entry.hostname
        );
    }

    let online = results
        .iter()
        .filter(|r| r.health_status.as_deref() == Some("ONLINE"))
        .count();

    println!("\n  Summary: {}/{} online", online, credentialed.len());

    if !errors.is_empty() {
        println!("\n  Errors ({}):", errors.len());
        for e in &errors {
            println!("    ! {}", e);
        }
    }

    println!("\n{}\n", "=".repeat(60));

    collector.stop();
}

// ============================================================================
// Diagnostic: Combined full status check
// ============================================================================

#[test]
#[ignore] // Requires network access and saved hosts
fn test_full_status_diagnostic() {
    println!("\n{}", "=".repeat(60));
    println!("  Full SSH & Health Status Diagnostic");
    println!("{}\n", "=".repeat(60));

    let Some((host_list, entries)) = load_saved_hosts() else {
        println!("SKIP: Cannot load SSH hosts.");
        return;
    };

    if entries.is_empty() {
        println!("SKIP: No saved SSH hosts found.");
        println!("  Add hosts to {:?}", ratterm::SSHStorage::default_path());
        return;
    }

    let credentialed: Vec<&HostEntry> = entries.iter().filter(|e| e.has_credentials).collect();
    println!("Storage: {:?}", ratterm::SSHStorage::default_path());
    println!(
        "Hosts:   {} total, {} with credentials\n",
        entries.len(),
        credentialed.len()
    );

    for entry in &entries {
        let marker = if entry.has_credentials { "*" } else { " " };
        println!("  {} {} ({})", marker, entry.display, entry.hostname);
    }
    println!();

    // Phase 1: TCP reachability for ALL hosts.
    println!("--- SSH Reachability (TCP port 22) ---");
    let mut reachability: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();

    for entry in &entries {
        let reachable = ratterm::NetworkScanner::check_host(&entry.hostname, entry.port);
        reachability.insert(entry.id, reachable);

        let label = if entry.display == entry.hostname {
            entry.hostname.clone()
        } else {
            format!("{} ({})", entry.display, entry.hostname)
        };

        if reachable {
            println!("  [OK] {}: port {} open", label, entry.port);
        } else {
            println!("  [!!] {}: port {} unreachable", label, entry.port);
        }
    }

    let reachable_count = reachability.values().filter(|&&v| v).count();
    println!("\n  Reachable: {}/{}\n", reachable_count, entries.len());

    // Phase 2: Health metrics for credentialed hosts.
    let mut health_errors: Vec<String> = Vec::new();

    if credentialed.is_empty() {
        println!("--- Health Metrics ---");
        println!("  (no credentialed hosts, skipping)\n");
    } else {
        println!("--- Health Metrics (SSH) ---");

        let collection_info = ratterm::build_collection_info(&host_list);
        let mut collector = ratterm::MetricsCollector::new();
        collector.collect(&collection_info);

        println!(
            "  Collecting metrics ({}s timeout)...",
            COLLECTION_WAIT_SECS
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(COLLECTION_WAIT_SECS);
        while !collector.is_collection_complete() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(250));
            collector.poll_results();
        }
        collector.poll_results();
        println!("  Collection finished.\n");

        for entry in &credentialed {
            let label = if entry.display == entry.hostname {
                entry.hostname.clone()
            } else {
                format!("{} ({})", entry.display, entry.hostname)
            };

            match collector.get_metrics(entry.id) {
                Some(m) if m.status.is_online() => {
                    println!(
                        "  [OK] {}: ONLINE (CPU: {:.0}%, RAM: {:.0}%, DISK: {:.0}%)",
                        label,
                        m.cpu_usage_percent,
                        m.memory_percent(),
                        m.disk_percent(),
                    );
                }
                Some(m) => {
                    let detail = m.error.as_deref().unwrap_or("unknown reason");
                    println!("  [!!] {}: {} ({})", label, m.status.as_str(), detail);
                    health_errors.push(format!("{}: {} ({})", label, m.status.as_str(), detail));
                }
                None => {
                    println!("  [!!] {}: no metrics collected", label);
                    health_errors.push(format!("{}: no metrics collected", label));
                }
            }
        }

        // Report skipped hosts.
        for entry in entries.iter().filter(|e| !e.has_credentials) {
            println!(
                "  [--] {} ({}): skipped (no credentials)",
                entry.display, entry.hostname
            );
        }

        let online = credentialed
            .iter()
            .filter(|e| {
                collector
                    .get_metrics(e.id)
                    .is_some_and(|m| m.status.is_online())
            })
            .count();

        println!("\n  Online: {}/{}\n", online, credentialed.len());

        collector.stop();
    }

    // Final report.
    let ssh_errors: Vec<String> = entries
        .iter()
        .filter(|e| !reachability.get(&e.id).copied().unwrap_or(false))
        .map(|e| {
            if e.display == e.hostname {
                format!("{}: Unreachable", e.hostname)
            } else {
                format!("{} ({}): Unreachable", e.display, e.hostname)
            }
        })
        .collect();

    let total_errors = ssh_errors.len() + health_errors.len();
    println!("{}", "=".repeat(60));

    if total_errors == 0 {
        println!("  All hosts OK");
    } else {
        println!("  Errors ({}):", total_errors);
        for e in ssh_errors.iter().chain(health_errors.iter()) {
            println!("    ! {}", e);
        }
    }

    println!("{}\n", "=".repeat(60));
}
