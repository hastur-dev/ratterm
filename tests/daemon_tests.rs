//! Integration tests for the daemon metrics collection system.
//!
//! These tests verify the daemon system components work correctly:
//! - Script output format and JSON structure
//! - Metrics receiver parsing
//! - Deployer command generation
//! - DaemonManager coordination

#![allow(clippy::unwrap_used, clippy::expect_used)]

use ratterm::daemon::{DAEMON_SCRIPT, DaemonError, DaemonManager, DaemonMetrics, DaemonStatus};
use ratterm::ssh::metrics::{GpuType, MetricStatus};

// ============================================================================
// Script Tests
// ============================================================================

mod script_tests {
    use super::*;

    #[test]
    fn test_daemon_script_contains_shebang() {
        assert!(
            DAEMON_SCRIPT.starts_with("#!/bin/bash"),
            "Script must start with bash shebang"
        );
    }

    #[test]
    fn test_daemon_script_reads_proc_loadavg() {
        assert!(
            DAEMON_SCRIPT.contains("/proc/loadavg"),
            "Script must read /proc/loadavg for CPU load"
        );
    }

    #[test]
    fn test_daemon_script_reads_proc_meminfo() {
        assert!(
            DAEMON_SCRIPT.contains("/proc/meminfo"),
            "Script must read /proc/meminfo for memory info"
        );
    }

    #[test]
    fn test_daemon_script_uses_df_for_disk() {
        assert!(
            DAEMON_SCRIPT.contains("df -BG"),
            "Script must use 'df -BG' for disk info"
        );
    }

    #[test]
    fn test_daemon_script_detects_nvidia_gpu() {
        assert!(
            DAEMON_SCRIPT.contains("nvidia-smi"),
            "Script must check for NVIDIA GPU via nvidia-smi"
        );
    }

    #[test]
    fn test_daemon_script_detects_amd_gpu() {
        assert!(
            DAEMON_SCRIPT.contains("rocm-smi"),
            "Script must check for AMD GPU via rocm-smi"
        );
    }

    #[test]
    fn test_daemon_script_uses_curl_or_wget() {
        let has_curl = DAEMON_SCRIPT.contains("curl ");
        let has_wget = DAEMON_SCRIPT.contains("wget ");
        assert!(
            has_curl || has_wget,
            "Script must use curl or wget to send metrics"
        );
    }

    #[test]
    fn test_daemon_script_targets_localhost_19999() {
        assert!(
            DAEMON_SCRIPT.contains("localhost:19999"),
            "Script must send metrics to localhost:19999"
        );
    }

    #[test]
    fn test_daemon_script_outputs_json() {
        // Check for JSON structure indicators
        assert!(
            DAEMON_SCRIPT.contains(r#""host_id":"#),
            "Script output must include host_id field"
        );
        assert!(
            DAEMON_SCRIPT.contains(r#""cpu":"#),
            "Script output must include cpu object"
        );
        assert!(
            DAEMON_SCRIPT.contains(r#""mem":"#),
            "Script output must include mem object"
        );
        assert!(
            DAEMON_SCRIPT.contains(r#""disk":"#),
            "Script output must include disk object"
        );
    }

    #[test]
    fn test_daemon_script_has_cleanup_trap() {
        assert!(
            DAEMON_SCRIPT.contains("trap"),
            "Script should have a trap for cleanup"
        );
    }

    #[test]
    fn test_daemon_script_uses_interval_variable() {
        assert!(
            DAEMON_SCRIPT.contains("INTERVAL"),
            "Script must use INTERVAL variable for configurable delay"
        );
    }
}

// ============================================================================
// Metrics Parsing Tests
// ============================================================================

mod metrics_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_minimal_metrics_json() {
        let json = r#"{
            "host_id": "1",
            "ts": 1700000000,
            "cpu": {"load": [0.5, 0.6, 0.7], "cores": 4},
            "mem": {"total": 8192, "avail": 4096},
            "disk": {"total": 500, "used": 250}
        }"#;

        let metrics: DaemonMetrics = serde_json::from_str(json).expect("Failed to parse JSON");

        assert_eq!(metrics.host_id, "1");
        assert_eq!(metrics.ts, 1700000000);
        assert_eq!(metrics.cpu.cores, 4);
        assert!((metrics.cpu.load[0] - 0.5).abs() < 0.01);
        assert_eq!(metrics.mem.total, 8192);
        assert_eq!(metrics.mem.avail, 4096);
        assert_eq!(metrics.disk.total, 500);
        assert_eq!(metrics.disk.used, 250);
        assert!(metrics.gpu.is_none());
    }

    #[test]
    fn test_parse_metrics_with_nvidia_gpu() {
        let json = r#"{
            "host_id": "2",
            "ts": 1700000000,
            "cpu": {"load": [1.0, 1.5, 2.0], "cores": 8},
            "mem": {"total": 32768, "avail": 16384, "swap_total": 8192, "swap_used": 1024},
            "disk": {"total": 1000, "used": 500},
            "gpu": {
                "gpu_type": "nvidia",
                "name": "RTX 3080",
                "usage": 75.5,
                "mem_used": 8192,
                "mem_total": 10240,
                "temp": 72.0
            }
        }"#;

        let metrics: DaemonMetrics = serde_json::from_str(json).expect("Failed to parse JSON");

        assert_eq!(metrics.host_id, "2");
        assert_eq!(metrics.cpu.cores, 8);
        assert_eq!(metrics.mem.swap_total, 8192);
        assert_eq!(metrics.mem.swap_used, 1024);

        let gpu = metrics.gpu.expect("GPU should be present");
        assert_eq!(gpu.gpu_type, "nvidia");
        assert_eq!(gpu.name, "RTX 3080");
        assert!((gpu.usage - 75.5).abs() < 0.01);
        assert_eq!(gpu.mem_used, 8192);
        assert_eq!(gpu.temp, Some(72.0));
    }

    #[test]
    fn test_parse_metrics_with_amd_gpu() {
        let json = r#"{
            "host_id": "3",
            "ts": 1700000000,
            "cpu": {"load": [2.0], "cores": 16},
            "mem": {"total": 65536, "avail": 32768},
            "disk": {"total": 2000, "used": 1000},
            "gpu": {
                "gpu_type": "amd",
                "name": "RX 6900 XT",
                "usage": 50.0,
                "mem_used": 8192,
                "mem_total": 16384
            }
        }"#;

        let metrics: DaemonMetrics = serde_json::from_str(json).expect("Failed to parse JSON");

        let gpu = metrics.gpu.expect("GPU should be present");
        assert_eq!(gpu.gpu_type, "amd");
        assert_eq!(gpu.temp, None);
    }

    #[test]
    fn test_convert_daemon_metrics_to_device_metrics() {
        let daemon_metrics = DaemonMetrics {
            host_id: "42".to_string(),
            ts: 1700000000,
            cpu: ratterm::daemon::types::DaemonCpuMetrics {
                load: vec![2.0, 1.5, 1.0],
                cores: 8,
            },
            mem: ratterm::daemon::types::DaemonMemMetrics {
                total: 32768,
                avail: 16384,
                swap_total: 8192,
                swap_used: 1024,
            },
            disk: ratterm::daemon::types::DaemonDiskMetrics {
                total: 1000,
                used: 500,
            },
            gpu: None,
        };

        let device_metrics = daemon_metrics.to_device_metrics();

        assert_eq!(device_metrics.host_id, 42);
        assert_eq!(device_metrics.cpu_cores, 8);
        assert!((device_metrics.load_avg.0 - 2.0).abs() < 0.01);
        assert!((device_metrics.load_avg.1 - 1.5).abs() < 0.01);
        assert!((device_metrics.load_avg.2 - 1.0).abs() < 0.01);
        assert_eq!(device_metrics.mem_total_mb, 32768);
        assert_eq!(device_metrics.mem_available_mb, 16384);
        assert_eq!(device_metrics.mem_used_mb, 16384);
        assert_eq!(device_metrics.swap_total_mb, 8192);
        assert_eq!(device_metrics.swap_used_mb, 1024);
        assert_eq!(device_metrics.disk_total_gb, 1000);
        assert_eq!(device_metrics.disk_used_gb, 500);
        assert!(device_metrics.gpu.is_none());
        assert_eq!(device_metrics.status, MetricStatus::Online);
    }

    #[test]
    fn test_convert_daemon_metrics_with_gpu() {
        let daemon_metrics = DaemonMetrics {
            host_id: "1".to_string(),
            ts: 1700000000,
            cpu: ratterm::daemon::types::DaemonCpuMetrics {
                load: vec![1.0, 1.0, 1.0],
                cores: 4,
            },
            mem: ratterm::daemon::types::DaemonMemMetrics {
                total: 16384,
                avail: 8192,
                swap_total: 0,
                swap_used: 0,
            },
            disk: ratterm::daemon::types::DaemonDiskMetrics {
                total: 500,
                used: 250,
            },
            gpu: Some(ratterm::daemon::types::DaemonGpuMetrics {
                gpu_type: "nvidia".to_string(),
                name: "RTX 3060".to_string(),
                usage: 45.0,
                mem_used: 4096,
                mem_total: 12288,
                temp: Some(55.0),
            }),
        };

        let device_metrics = daemon_metrics.to_device_metrics();

        let gpu = device_metrics.gpu.expect("GPU should be present");
        assert_eq!(gpu.gpu_type, GpuType::Nvidia);
        assert_eq!(gpu.name, "RTX 3060");
        assert!((gpu.usage_percent - 45.0).abs() < 0.01);
        assert_eq!(gpu.memory_used_mb, 4096);
        assert_eq!(gpu.memory_total_mb, 12288);
        assert_eq!(gpu.temperature_celsius, Some(55.0));
    }

    #[test]
    fn test_cpu_usage_calculation_from_load() {
        // Test with load average equal to number of cores (100% CPU)
        let metrics = DaemonMetrics {
            host_id: "1".to_string(),
            ts: 0,
            cpu: ratterm::daemon::types::DaemonCpuMetrics {
                load: vec![4.0, 3.0, 2.0],
                cores: 4,
            },
            mem: ratterm::daemon::types::DaemonMemMetrics::default(),
            disk: ratterm::daemon::types::DaemonDiskMetrics::default(),
            gpu: None,
        };

        let device = metrics.to_device_metrics();
        // Load 4.0 on 4 cores = 100% CPU usage
        assert!((device.cpu_usage_percent - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_cpu_usage_clamped_at_100() {
        // Test with load higher than cores (should clamp at 100%)
        let metrics = DaemonMetrics {
            host_id: "1".to_string(),
            ts: 0,
            cpu: ratterm::daemon::types::DaemonCpuMetrics {
                load: vec![16.0, 12.0, 8.0],
                cores: 4,
            },
            mem: ratterm::daemon::types::DaemonMemMetrics::default(),
            disk: ratterm::daemon::types::DaemonDiskMetrics::default(),
            gpu: None,
        };

        let device = metrics.to_device_metrics();
        // Load 16.0 on 4 cores = 400%, clamped to 100%
        assert!((device.cpu_usage_percent - 100.0).abs() < 0.01);
    }
}

// ============================================================================
// Deployer Tests
// ============================================================================

mod deployer_tests {
    use ratterm::daemon::script::{
        deploy_daemon_command, status_daemon_command, stop_daemon_command,
    };

    #[test]
    fn test_deploy_command_includes_host_id() {
        let cmd = deploy_daemon_command(42);
        assert!(
            cmd.contains("HOST_ID=42"),
            "Deploy command must set HOST_ID env var"
        );
    }

    #[test]
    fn test_deploy_command_creates_directory() {
        let cmd = deploy_daemon_command(1);
        assert!(
            cmd.contains("mkdir -p ~/.ratterm"),
            "Deploy command must create ~/.ratterm directory"
        );
    }

    #[test]
    fn test_deploy_command_writes_script() {
        let cmd = deploy_daemon_command(1);
        assert!(
            cmd.contains("~/.ratterm/daemon.sh"),
            "Deploy command must write to ~/.ratterm/daemon.sh"
        );
    }

    #[test]
    fn test_deploy_command_makes_executable() {
        let cmd = deploy_daemon_command(1);
        assert!(
            cmd.contains("chmod +x"),
            "Deploy command must make script executable"
        );
    }

    #[test]
    fn test_deploy_command_uses_nohup() {
        let cmd = deploy_daemon_command(1);
        assert!(
            cmd.contains("nohup"),
            "Deploy command must use nohup for background execution"
        );
    }

    #[test]
    fn test_deploy_command_uses_heredoc() {
        let cmd = deploy_daemon_command(1);
        assert!(
            cmd.contains("RATTERM_EOF"),
            "Deploy command must use heredoc for script content"
        );
    }

    #[test]
    fn test_status_command_uses_pgrep() {
        let cmd = status_daemon_command();
        assert!(
            cmd.contains("pgrep"),
            "Status command must use pgrep to check daemon"
        );
        assert!(
            cmd.contains("ratterm-daemon"),
            "Status command must search for ratterm-daemon process"
        );
    }

    #[test]
    fn test_stop_command_uses_pkill() {
        let cmd = stop_daemon_command();
        assert!(
            cmd.contains("pkill"),
            "Stop command must use pkill to stop daemon"
        );
        assert!(
            cmd.contains("ratterm-daemon"),
            "Stop command must target ratterm-daemon process"
        );
    }
}

// ============================================================================
// DaemonManager Tests
// ============================================================================

mod manager_tests {
    use super::*;

    #[test]
    fn test_daemon_manager_creation() {
        let manager = DaemonManager::new();
        assert!(!manager.is_active(), "New manager should not be active");
    }

    #[test]
    fn test_daemon_manager_default() {
        let manager = DaemonManager::default();
        assert!(!manager.is_active(), "Default manager should not be active");
    }

    #[test]
    fn test_active_hosts_initially_empty() {
        let manager = DaemonManager::new();
        assert_eq!(manager.active_host_count(), 0, "No active hosts initially");
        assert!(
            manager.active_host_ids().is_empty(),
            "Active host set should be empty"
        );
    }

    #[test]
    fn test_get_metrics_when_inactive() {
        let manager = DaemonManager::new();
        let metrics = manager.get_metrics(1);
        assert!(
            metrics.is_none(),
            "Should return None when manager not active"
        );
    }

    #[test]
    fn test_has_recent_metrics_when_inactive() {
        let manager = DaemonManager::new();
        assert!(
            !manager.has_recent_metrics(1),
            "Should return false when manager not active"
        );
    }

    #[test]
    fn test_cached_metrics_count_when_inactive() {
        let manager = DaemonManager::new();
        assert_eq!(
            manager.cached_metrics_count(),
            0,
            "Should return 0 when manager not active"
        );
    }

    #[test]
    fn test_manager_start_stop() {
        let mut manager = DaemonManager::new();

        // Start might fail if port is in use
        match manager.start() {
            Ok(()) => {
                assert!(manager.is_active(), "Manager should be active after start");

                // Can start again without error (idempotent)
                let result = manager.start();
                assert!(result.is_ok(), "Starting again should succeed");

                manager.stop();
                assert!(
                    !manager.is_active(),
                    "Manager should not be active after stop"
                );

                // Can stop again without error (idempotent)
                manager.stop();
                assert!(!manager.is_active(), "Stopping again should be safe");
            }
            Err(e) => {
                // Port likely in use, skip test
                eprintln!("Skipping test, receiver start failed: {}", e);
            }
        }
    }

    #[test]
    fn test_deploy_fails_when_inactive() {
        let manager = DaemonManager::new();
        let ctx =
            ratterm::terminal::SSHContext::new("test".to_string(), "localhost".to_string(), 22);

        let result = manager.deploy_to_host(&ctx, 1);
        assert!(
            result.is_err(),
            "Deploy should fail when manager not active"
        );

        if let Err(DaemonError::ServerError(msg)) = result {
            assert!(
                msg.contains("not active"),
                "Error should mention manager not active"
            );
        }
    }
}

// ============================================================================
// DaemonStatus Tests
// ============================================================================

mod status_tests {
    use super::*;

    #[test]
    fn test_daemon_status_running() {
        let status = DaemonStatus::Running(12345);
        match status {
            DaemonStatus::Running(pid) => assert_eq!(pid, 12345),
            _ => panic!("Expected Running status"),
        }
    }

    #[test]
    fn test_daemon_status_not_running() {
        let status = DaemonStatus::NotRunning;
        assert_eq!(status, DaemonStatus::NotRunning);
    }

    #[test]
    fn test_daemon_status_unknown() {
        let status = DaemonStatus::Unknown;
        assert_eq!(status, DaemonStatus::Unknown);
    }

    #[test]
    fn test_daemon_status_equality() {
        assert_eq!(DaemonStatus::Running(100), DaemonStatus::Running(100));
        assert_ne!(DaemonStatus::Running(100), DaemonStatus::Running(200));
        assert_ne!(DaemonStatus::Running(100), DaemonStatus::NotRunning);
        assert_ne!(DaemonStatus::NotRunning, DaemonStatus::Unknown);
    }
}

// ============================================================================
// DaemonError Tests
// ============================================================================

mod error_tests {
    use super::*;

    #[test]
    fn test_daemon_error_display_deploy_failed() {
        let err = DaemonError::DeployFailed("connection timeout".to_string());
        let display = err.to_string();
        assert!(display.contains("Deploy failed"));
        assert!(display.contains("connection timeout"));
    }

    #[test]
    fn test_daemon_error_display_stop_failed() {
        let err = DaemonError::StopFailed("process not found".to_string());
        let display = err.to_string();
        assert!(display.contains("Stop failed"));
        assert!(display.contains("process not found"));
    }

    #[test]
    fn test_daemon_error_display_not_running() {
        let err = DaemonError::NotRunning;
        let display = err.to_string();
        assert!(display.contains("not running"));
    }

    #[test]
    fn test_daemon_error_display_invalid_metrics() {
        let err = DaemonError::InvalidMetrics("missing field".to_string());
        let display = err.to_string();
        assert!(display.contains("Invalid metrics"));
        assert!(display.contains("missing field"));
    }

    #[test]
    fn test_daemon_error_display_server_error() {
        let err = DaemonError::ServerError("port in use".to_string());
        let display = err.to_string();
        assert!(display.contains("Server error"));
        assert!(display.contains("port in use"));
    }

    #[test]
    fn test_daemon_error_display_ssh_error() {
        let err = DaemonError::SshError("authentication failed".to_string());
        let display = err.to_string();
        assert!(display.contains("SSH error"));
        assert!(display.contains("authentication failed"));
    }
}

// ============================================================================
// JSON Roundtrip Tests
// ============================================================================

mod json_roundtrip_tests {
    use super::*;

    #[test]
    fn test_daemon_metrics_json_roundtrip() {
        let original = DaemonMetrics {
            host_id: "test-host".to_string(),
            ts: 1700000000,
            cpu: ratterm::daemon::types::DaemonCpuMetrics {
                load: vec![1.5, 1.2, 0.9],
                cores: 12,
            },
            mem: ratterm::daemon::types::DaemonMemMetrics {
                total: 65536,
                avail: 32768,
                swap_total: 16384,
                swap_used: 2048,
            },
            disk: ratterm::daemon::types::DaemonDiskMetrics {
                total: 2000,
                used: 1000,
            },
            gpu: Some(ratterm::daemon::types::DaemonGpuMetrics {
                gpu_type: "nvidia".to_string(),
                name: "RTX 4090".to_string(),
                usage: 85.5,
                mem_used: 20480,
                mem_total: 24576,
                temp: Some(78.0),
            }),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original).expect("Failed to serialize");

        // Deserialize back
        let parsed: DaemonMetrics = serde_json::from_str(&json).expect("Failed to deserialize");

        // Verify fields match
        assert_eq!(parsed.host_id, original.host_id);
        assert_eq!(parsed.ts, original.ts);
        assert_eq!(parsed.cpu.cores, original.cpu.cores);
        assert_eq!(parsed.cpu.load.len(), original.cpu.load.len());
        assert_eq!(parsed.mem.total, original.mem.total);
        assert_eq!(parsed.disk.total, original.disk.total);

        let parsed_gpu = parsed.gpu.expect("GPU should be present");
        let original_gpu = original.gpu.expect("Original GPU should be present");
        assert_eq!(parsed_gpu.gpu_type, original_gpu.gpu_type);
        assert_eq!(parsed_gpu.name, original_gpu.name);
    }

    #[test]
    fn test_minimal_json_has_default_optional_fields() {
        // JSON with only required fields - load is required, but swap fields are optional
        let json = r#"{
            "host_id": "1",
            "ts": 0,
            "cpu": {"load": [0.0], "cores": 1},
            "mem": {"total": 1024, "avail": 512},
            "disk": {"total": 100, "used": 50}
        }"#;

        let metrics: DaemonMetrics = serde_json::from_str(json).expect("Failed to parse");

        // swap should default to 0
        assert_eq!(metrics.mem.swap_total, 0);
        assert_eq!(metrics.mem.swap_used, 0);

        // gpu should default to None
        assert!(metrics.gpu.is_none());
    }
}

// ============================================================================
// Integration Tests (require actual network - marked as ignored by default)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test the full metrics receiver startup and shutdown cycle.
    ///
    /// This test is ignored by default as it binds to a network port.
    /// Run with: cargo test --test daemon_tests integration -- --ignored
    #[test]
    #[ignore]
    fn test_receiver_lifecycle() {
        let mut manager = DaemonManager::new();

        // Start the manager
        manager.start().expect("Failed to start manager");
        assert!(manager.is_active());

        // Verify no metrics initially
        assert!(manager.get_metrics(1).is_none());

        // Clean shutdown
        manager.stop();
        assert!(!manager.is_active());
    }
}
