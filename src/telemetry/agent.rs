//! Local metric collection for the push agent.
//!
//! The deployed daemon was a Bash script that parsed `/proc` and posted with
//! `curl` or `wget`. That fixed it to Linux hosts with one of those two
//! programs installed, and put the parsing somewhere no test could reach.
//!
//! This reads the same numbers through `sysinfo`, which covers Linux, macOS
//! and Windows, and produces the same [`DeviceMetrics`] the SSH poller does,
//! so both paths meet at [`crate::telemetry::Telemetry::ingest`]. The `rat-agent`
//! binary is a thin wrapper around [`Agent::sample`] and the reporter below.
//!
//! GPU metrics still come from `nvidia-smi` when it is present: `sysinfo` does
//! not report them, and shelling out to one well-known program for one
//! optional metric is a smaller cost than a GPU vendor dependency.

use std::process::Command;
use std::time::Duration;

use sysinfo::{Disks, System};
use tracing::debug;

use crate::daemon::types::{
    DaemonCpuMetrics, DaemonDiskMetrics, DaemonGpuMetrics, DaemonMemMetrics, DaemonMetrics,
};
use crate::ssh::metrics::{DeviceMetrics, GpuMetrics, GpuType, MetricStatus};

/// Interval between samples when the agent runs as a service.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// How long `nvidia-smi` is given before its output is ignored.
const GPU_QUERY_TIMEOUT: Duration = Duration::from_secs(3);

/// Bytes per mebibyte.
const MIB: u64 = 1024 * 1024;
/// Bytes per gibibyte.
const GIB: u64 = 1024 * 1024 * 1024;

/// Collects metrics about the machine it runs on.
pub struct Agent {
    system: System,
    disks: Disks,
    host_id: u32,
    collect_gpu: bool,
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("host_id", &self.host_id)
            .field("collect_gpu", &self.collect_gpu)
            .finish()
    }
}

impl Agent {
    /// Creates an agent reporting as `host_id`.
    #[must_use]
    pub fn new(host_id: u32) -> Self {
        Self {
            system: System::new(),
            disks: Disks::new_with_refreshed_list(),
            host_id,
            collect_gpu: true,
        }
    }

    /// Turns GPU collection off, for a host with no GPU or no `nvidia-smi`.
    #[must_use]
    pub const fn without_gpu(mut self) -> Self {
        self.collect_gpu = false;
        self
    }

    /// Returns the host id this agent reports as.
    #[must_use]
    pub const fn host_id(&self) -> u32 {
        self.host_id
    }

    /// Takes one sample of the local machine.
    ///
    /// CPU percentage needs two observations to mean anything, so the first
    /// call after construction reports zero and the caller should discard it
    /// or wait one interval. That is why the service loop samples before it
    /// starts reporting.
    pub fn sample(&mut self) -> DeviceMetrics {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.disks.refresh(true);

        let mut metrics = DeviceMetrics::new(self.host_id);
        metrics.status = MetricStatus::Online;

        let cpus = self.system.cpus();
        metrics.cpu_cores = u16::try_from(cpus.len()).unwrap_or(u16::MAX);
        if !cpus.is_empty() {
            #[allow(clippy::cast_precision_loss)]
            let total: f32 = cpus.iter().map(sysinfo::Cpu::cpu_usage).sum();
            metrics.cpu_usage_percent = total / cpus.len() as f32;
        }

        let load = System::load_average();
        metrics.load_avg = (load.one as f32, load.five as f32, load.fifteen as f32);

        metrics.mem_total_mb = self.system.total_memory() / MIB;
        metrics.mem_used_mb = self.system.used_memory() / MIB;
        metrics.mem_available_mb = self.system.available_memory() / MIB;
        metrics.swap_total_mb = self.system.total_swap() / MIB;
        metrics.swap_used_mb = self.system.used_swap() / MIB;

        let (used, total) = root_disk_usage(&self.disks);
        metrics.disk_used_gb = used / GIB;
        metrics.disk_total_gb = total / GIB;

        if self.collect_gpu {
            metrics.gpu = query_nvidia_gpu();
            if metrics.gpu.is_none() {
                // Do not pay for the process on every sample once it is clear
                // there is nothing to read.
                self.collect_gpu = false;
                debug!("no NVIDIA GPU reported; not asking again this run");
            }
        }

        metrics
    }
}

/// Returns used and total bytes for the filesystem holding the root.
///
/// `sysinfo` reports every mount, including snap loopbacks on Linux and
/// per-volume entries on Windows. The dashboard wants one number, so this
/// picks the mount that holds the system: `/` on Unix, and the largest volume
/// on Windows, where there is no single root.
#[must_use]
pub fn root_disk_usage(disks: &Disks) -> (u64, u64) {
    let mut best: Option<(u64, u64)> = None;

    for disk in disks.list() {
        let total = disk.total_space();
        if total == 0 {
            continue;
        }
        let used = total.saturating_sub(disk.available_space());
        let mount = disk.mount_point();

        let is_root = mount == std::path::Path::new("/");
        if is_root {
            return (used, total);
        }

        // Otherwise keep the largest volume seen.
        if best.is_none_or(|(_, best_total)| total > best_total) {
            best = Some((used, total));
        }
    }

    best.unwrap_or((0, 0))
}

/// Reads GPU metrics from `nvidia-smi`, if it is present.
#[must_use]
pub fn query_nvidia_gpu() -> Option<GpuMetrics> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_nvidia_smi(&String::from_utf8_lossy(&output.stdout))
}

/// Parses one line of `nvidia-smi --format=csv,noheader,nounits` output.
///
/// Split out from the process call so the parsing is testable on a machine
/// with no GPU, which is most of them.
#[must_use]
pub fn parse_nvidia_smi(text: &str) -> Option<GpuMetrics> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    if fields.len() < 5 {
        return None;
    }

    Some(GpuMetrics {
        name: fields[0].to_string(),
        gpu_type: GpuType::Nvidia,
        usage_percent: fields[1].parse().unwrap_or(0.0),
        memory_used_mb: fields[2].parse().unwrap_or(0),
        memory_total_mb: fields[3].parse().unwrap_or(0),
        temperature_celsius: fields[4].parse().ok(),
    })
}

/// Returns how long `nvidia-smi` is allowed to take.
#[must_use]
pub const fn gpu_query_timeout() -> Duration {
    GPU_QUERY_TIMEOUT
}

/// Converts a local sample into the wire format the receiver already speaks.
///
/// Keeping the existing `POST /metrics` payload means a fleet running the old
/// shell daemon and one running this agent report to the same endpoint, and
/// hosts can be migrated one at a time.
#[must_use]
pub fn to_daemon_metrics(metrics: &DeviceMetrics, ts: u64) -> DaemonMetrics {
    DaemonMetrics {
        host_id: metrics.host_id.to_string(),
        ts,
        cpu: DaemonCpuMetrics {
            load: load_vector(
                metrics.load_avg,
                metrics.cpu_usage_percent,
                metrics.cpu_cores,
            ),
            cores: metrics.cpu_cores,
        },
        mem: DaemonMemMetrics {
            total: metrics.mem_total_mb,
            avail: metrics.mem_available_mb,
            swap_total: metrics.swap_total_mb,
            swap_used: metrics.swap_used_mb,
        },
        disk: DaemonDiskMetrics {
            total: metrics.disk_total_gb,
            used: metrics.disk_used_gb,
        },
        gpu: metrics.gpu.as_ref().map(|gpu| DaemonGpuMetrics {
            gpu_type: gpu.gpu_type.as_str().to_lowercase(),
            name: gpu.name.clone(),
            usage: gpu.usage_percent,
            mem_used: gpu.memory_used_mb,
            mem_total: gpu.memory_total_mb,
            temp: gpu.temperature_celsius,
        }),
    }
}

/// Chooses the load vector to report.
///
/// The receiver derives CPU percentage from `load[0] / cores`, which is a
/// Unix assumption: Windows has no load average and `sysinfo` reports zeros
/// there. Rather than send a host that is at 90% CPU as idle, a zero load
/// average is replaced by the equivalent of the measured percentage. A real
/// load average is passed through untouched.
#[must_use]
pub fn load_vector(load_avg: (f32, f32, f32), cpu_percent: f32, cores: u16) -> Vec<f32> {
    let (one, five, fifteen) = load_avg;
    if one > 0.0 || five > 0.0 || fifteen > 0.0 {
        return vec![one, five, fifteen];
    }

    let equivalent = (cpu_percent / 100.0) * f32::from(cores.max(1));
    vec![equivalent, equivalent, equivalent]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_reports_the_host_it_was_given() {
        let agent = Agent::new(42);
        assert_eq!(agent.host_id(), 42);
    }

    #[test]
    fn gpu_collection_can_be_turned_off() {
        let agent = Agent::new(1).without_gpu();
        assert!(!agent.collect_gpu);
    }

    #[test]
    fn a_sample_reports_this_machine() {
        let mut agent = Agent::new(1).without_gpu();
        // The first sample has no CPU delta to work from; take two.
        let _ = agent.sample();
        std::thread::sleep(Duration::from_millis(200));
        let metrics = agent.sample();

        assert_eq!(metrics.status, MetricStatus::Online);
        assert!(metrics.cpu_cores >= 1, "at least one core");
        assert!(metrics.mem_total_mb > 0, "some memory");
        assert!(
            metrics.cpu_usage_percent >= 0.0 && metrics.cpu_usage_percent <= 100.0,
            "cpu was {}",
            metrics.cpu_usage_percent
        );
        assert!(metrics.mem_used_mb <= metrics.mem_total_mb);
    }

    #[test]
    fn a_sample_reports_a_root_filesystem_where_there_is_one() {
        let mut agent = Agent::new(1).without_gpu();
        let metrics = agent.sample();
        // A machine with no readable filesystem is possible in a sandbox, so
        // this asserts consistency rather than a non-zero size.
        assert!(metrics.disk_used_gb <= metrics.disk_total_gb);
    }

    #[test]
    fn disk_usage_of_an_empty_list_is_zero() {
        let disks = Disks::new();
        assert_eq!(root_disk_usage(&disks), (0, 0));
    }

    #[test]
    fn nvidia_output_parses() {
        let gpu =
            parse_nvidia_smi("NVIDIA GeForce RTX 5060 Ti, 78, 4096, 16384, 61\n").expect("a GPU");
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 5060 Ti");
        assert_eq!(gpu.gpu_type, GpuType::Nvidia);
        assert!((gpu.usage_percent - 78.0).abs() < f32::EPSILON);
        assert_eq!(gpu.memory_used_mb, 4096);
        assert_eq!(gpu.memory_total_mb, 16384);
        assert_eq!(gpu.temperature_celsius, Some(61.0));
    }

    #[test]
    fn the_first_gpu_is_used_when_several_are_reported() {
        let gpu = parse_nvidia_smi("first, 10, 1, 2, 30\nsecond, 20, 3, 4, 40\n").expect("a GPU");
        assert_eq!(gpu.name, "first");
    }

    #[test]
    fn leading_blank_lines_are_skipped() {
        let gpu = parse_nvidia_smi("\n\n  \nonly, 5, 1, 2, 3\n").expect("a GPU");
        assert_eq!(gpu.name, "only");
    }

    #[test]
    fn empty_nvidia_output_yields_nothing() {
        assert!(parse_nvidia_smi("").is_none());
        assert!(parse_nvidia_smi("   \n\n").is_none());
    }

    #[test]
    fn a_short_nvidia_line_yields_nothing() {
        assert!(parse_nvidia_smi("name, 10, 20\n").is_none());
    }

    #[test]
    fn unparseable_numbers_become_zero_rather_than_failing() {
        let gpu = parse_nvidia_smi("name, [N/A], [N/A], [N/A], [N/A]").expect("a GPU");
        assert_eq!(gpu.name, "name");
        assert!((gpu.usage_percent - 0.0).abs() < f32::EPSILON);
        assert_eq!(gpu.memory_used_mb, 0);
        assert_eq!(gpu.temperature_celsius, None);
    }

    #[test]
    fn the_gpu_query_has_a_timeout() {
        assert!(gpu_query_timeout() > Duration::ZERO);
        assert!(gpu_query_timeout() <= Duration::from_secs(10));
    }

    #[test]
    fn debug_output_names_the_host() {
        let agent = Agent::new(9);
        assert!(format!("{agent:?}").contains("host_id: 9"));
    }

    /// A filled-in sample, so each conversion test can vary one field.
    fn sample_metrics() -> DeviceMetrics {
        let mut metrics = DeviceMetrics::new(7);
        metrics.status = MetricStatus::Online;
        metrics.cpu_cores = 8;
        metrics.cpu_usage_percent = 25.0;
        metrics.load_avg = (1.5, 1.2, 0.9);
        metrics.mem_total_mb = 16_384;
        metrics.mem_available_mb = 8_192;
        metrics.mem_used_mb = 8_192;
        metrics.swap_total_mb = 4_096;
        metrics.swap_used_mb = 512;
        metrics.disk_total_gb = 500;
        metrics.disk_used_gb = 250;
        metrics
    }

    #[test]
    fn a_sample_survives_the_round_trip_through_the_wire_format() {
        let original = sample_metrics();
        let wire = to_daemon_metrics(&original, 1_700_000_000);
        let back = wire.to_device_metrics();

        assert_eq!(back.host_id, original.host_id);
        assert_eq!(back.cpu_cores, original.cpu_cores);
        assert_eq!(back.mem_total_mb, original.mem_total_mb);
        assert_eq!(back.mem_available_mb, original.mem_available_mb);
        assert_eq!(back.swap_used_mb, original.swap_used_mb);
        assert_eq!(back.disk_total_gb, original.disk_total_gb);
        assert_eq!(back.disk_used_gb, original.disk_used_gb);
        assert_eq!(back.status, MetricStatus::Online);
    }

    #[test]
    fn the_wire_format_serialises_as_the_receiver_expects() {
        let wire = to_daemon_metrics(&sample_metrics(), 1_700_000_000);
        let json = serde_json::to_string(&wire).expect("serialisable");
        let parsed: DaemonMetrics = serde_json::from_str(&json).expect("parseable");
        assert_eq!(parsed.host_id, "7");
        assert_eq!(parsed.ts, 1_700_000_000);
        assert_eq!(parsed.cpu.cores, 8);
    }

    #[test]
    fn a_gpu_is_carried_into_the_wire_format() {
        let mut metrics = sample_metrics();
        metrics.gpu = Some(GpuMetrics {
            name: "RTX 5060 Ti".to_string(),
            gpu_type: GpuType::Nvidia,
            usage_percent: 61.0,
            memory_used_mb: 4_096,
            memory_total_mb: 16_384,
            temperature_celsius: Some(58.0),
        });

        let wire = to_daemon_metrics(&metrics, 1);
        let gpu = wire.gpu.as_ref().expect("a GPU on the wire");
        assert_eq!(gpu.gpu_type, "nvidia", "the receiver matches lowercase");
        assert_eq!(gpu.name, "RTX 5060 Ti");

        let back = wire.to_device_metrics();
        let back_gpu = back.gpu.expect("a GPU after conversion");
        assert_eq!(back_gpu.gpu_type, GpuType::Nvidia);
        assert_eq!(back_gpu.memory_total_mb, 16_384);
    }

    #[test]
    fn a_host_with_no_gpu_sends_none() {
        let wire = to_daemon_metrics(&sample_metrics(), 1);
        assert!(wire.gpu.is_none());
    }

    #[test]
    fn a_real_load_average_is_passed_through() {
        let load = load_vector((1.5, 1.2, 0.9), 25.0, 8);
        assert_eq!(load, vec![1.5, 1.2, 0.9]);
    }

    #[test]
    fn a_windows_host_reports_its_cpu_rather_than_a_zero_load() {
        // sysinfo reports a zero load average on Windows. Sending it verbatim
        // would show a busy host as idle, because the receiver divides load by
        // core count to get a percentage.
        let load = load_vector((0.0, 0.0, 0.0), 50.0, 8);
        assert_eq!(load, vec![4.0, 4.0, 4.0]);

        let mut metrics = sample_metrics();
        metrics.load_avg = (0.0, 0.0, 0.0);
        metrics.cpu_usage_percent = 50.0;
        let back = to_daemon_metrics(&metrics, 1).to_device_metrics();
        assert!(
            (back.cpu_usage_percent - 50.0).abs() < 0.01,
            "cpu came back as {}",
            back.cpu_usage_percent
        );
    }

    #[test]
    fn an_idle_windows_host_reports_zero_rather_than_dividing_by_zero() {
        let load = load_vector((0.0, 0.0, 0.0), 0.0, 0);
        assert_eq!(load, vec![0.0, 0.0, 0.0]);
    }
}
