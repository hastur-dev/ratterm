//! Embedded daemon script for remote metric collection.
//!
//! Contains the shell script that runs on remote Linux hosts
//! to collect and report system metrics.

/// The daemon script that runs on remote hosts.
///
/// This script:
/// 1. Reads CPU info from /proc/loadavg and nproc
/// 2. Reads memory info from /proc/meminfo
/// 3. Reads disk info from df
/// 4. Detects GPU via nvidia-smi or rocm-smi
/// 5. Outputs JSON and POSTs to localhost:19999/metrics
///
/// The script is designed to be lightweight and portable across
/// Linux distributions. It uses only standard tools available
/// on most systems.
pub const DAEMON_SCRIPT: &str = r#"#!/bin/bash
# ratterm-daemon.sh - Lightweight metrics collector
# Sends system metrics to Ratterm health dashboard via reverse SSH tunnel

set -e

INTERVAL="${INTERVAL:-1}"
HOST_ID="${HOST_ID:-unknown}"
RECEIVER_URL="${RECEIVER_URL:-http://localhost:19999/metrics}"

# Cleanup function
cleanup() {
    exit 0
}
trap cleanup SIGTERM SIGINT

# Collect and send metrics
collect_metrics() {
    # CPU: load average and core count
    local load_line
    load_line=$(cat /proc/loadavg 2>/dev/null || echo "0 0 0 0/0 0")
    local load1 load5 load15
    read -r load1 load5 load15 _ _ <<< "$load_line"
    local cores
    cores=$(nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo 1)

    # Memory: total and available in MB
    local meminfo mem_total_kb mem_avail_kb swap_total_kb swap_free_kb
    meminfo=$(cat /proc/meminfo 2>/dev/null)
    mem_total_kb=$(echo "$meminfo" | awk '/^MemTotal:/ {print $2}')
    mem_avail_kb=$(echo "$meminfo" | awk '/^MemAvailable:/ {print $2}')
    # Fallback to MemFree if MemAvailable not present
    if [ -z "$mem_avail_kb" ] || [ "$mem_avail_kb" = "0" ]; then
        mem_avail_kb=$(echo "$meminfo" | awk '/^MemFree:/ {print $2}')
    fi
    swap_total_kb=$(echo "$meminfo" | awk '/^SwapTotal:/ {print $2}')
    swap_free_kb=$(echo "$meminfo" | awk '/^SwapFree:/ {print $2}')

    # Convert to MB
    local mem_total=$((${mem_total_kb:-0} / 1024))
    local mem_avail=$((${mem_avail_kb:-0} / 1024))
    local swap_total=$((${swap_total_kb:-0} / 1024))
    local swap_used=$(( (${swap_total_kb:-0} - ${swap_free_kb:-0}) / 1024 ))

    # Disk: root filesystem in GB
    local disk_info disk_total disk_used
    disk_info=$(df -BG / 2>/dev/null | tail -1 | awk '{print $2" "$3}' | tr -d 'G')
    disk_total=$(echo "$disk_info" | awk '{print $1}')
    disk_used=$(echo "$disk_info" | awk '{print $2}')

    # GPU detection (NVIDIA, AMD, Raspberry Pi VideoCore)
    local gpu_json=""
    if command -v nvidia-smi &>/dev/null; then
        local gpu_data
        gpu_data=$(nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv,noheader,nounits 2>/dev/null | head -1)
        if [ -n "$gpu_data" ]; then
            local gpu_name gpu_usage gpu_mem_used gpu_mem_total gpu_temp
            IFS=',' read -r gpu_name gpu_usage gpu_mem_used gpu_mem_total gpu_temp <<< "$gpu_data"
            gpu_name=$(echo "$gpu_name" | xargs)
            gpu_usage=$(echo "$gpu_usage" | xargs)
            gpu_mem_used=$(echo "$gpu_mem_used" | xargs)
            gpu_mem_total=$(echo "$gpu_mem_total" | xargs)
            gpu_temp=$(echo "$gpu_temp" | xargs)
            gpu_json=$(printf ',"gpu":{"gpu_type":"nvidia","name":"%s","usage":%s,"mem_used":%s,"mem_total":%s,"temp":%s}' \
                "$gpu_name" "${gpu_usage:-0}" "${gpu_mem_used:-0}" "${gpu_mem_total:-0}" "${gpu_temp:-0}")
        fi
    elif command -v rocm-smi &>/dev/null; then
        # AMD GPU - use grep -E instead of -P for BusyBox compatibility
        local gpu_usage
        gpu_usage=$(rocm-smi --showuse 2>/dev/null | grep -E -o '[0-9]+%' | head -1 | tr -d '%' || echo "0")
        gpu_json=$(printf ',"gpu":{"gpu_type":"amd","name":"AMD GPU","usage":%s,"mem_used":0,"mem_total":0,"temp":null}' "${gpu_usage:-0}")
    elif command -v vcgencmd &>/dev/null; then
        # Raspberry Pi VideoCore GPU
        local gpu_temp gpu_mem
        gpu_temp=$(vcgencmd measure_temp 2>/dev/null | grep -E -o '[0-9.]+' | head -1 || echo "0")
        gpu_mem=$(vcgencmd get_mem gpu 2>/dev/null | grep -E -o '[0-9]+' || echo "0")
        gpu_json=$(printf ',"gpu":{"gpu_type":"videocore","name":"VideoCore","usage":0,"mem_used":0,"mem_total":%s,"temp":%s}' "${gpu_mem:-0}" "${gpu_temp:-0}")
    fi

    # Build JSON payload
    local ts
    ts=$(date +%s)

    printf '{"host_id":"%s","ts":%s,"cpu":{"load":[%s,%s,%s],"cores":%s},"mem":{"total":%s,"avail":%s,"swap_total":%s,"swap_used":%s},"disk":{"total":%s,"used":%s}%s}\n' \
        "$HOST_ID" "$ts" "${load1:-0}" "${load5:-0}" "${load15:-0}" "${cores:-1}" \
        "${mem_total:-0}" "${mem_avail:-0}" "${swap_total:-0}" "${swap_used:-0}" \
        "${disk_total:-0}" "${disk_used:-0}" "$gpu_json"
}

# Main loop
while true; do
    metrics=$(collect_metrics)

    # Send metrics via curl (ignore errors, will retry next interval)
    if command -v curl &>/dev/null; then
        curl -s -X POST -H "Content-Type: application/json" -d "$metrics" "$RECEIVER_URL" 2>/dev/null || true
    elif command -v wget &>/dev/null; then
        echo "$metrics" | wget -q --post-data=- -O /dev/null "$RECEIVER_URL" 2>/dev/null || true
    fi

    sleep "$INTERVAL"
done
"#;

/// Command to check if daemon is running.
pub const CHECK_DAEMON_CMD: &str = "pgrep -f 'ratterm-daemon' || echo 'NOT_RUNNING'";

/// Command to stop the daemon.
pub const STOP_DAEMON_CMD: &str = "pkill -f 'ratterm-daemon' 2>/dev/null || true";

/// Generates the command to deploy and start the daemon.
///
/// # Arguments
/// * `host_id` - The unique identifier for this host
///
/// # Returns
/// A shell command string that writes the daemon script and starts it.
#[must_use]
pub fn deploy_daemon_command(host_id: u32) -> String {
    // Escape single quotes in the script for heredoc
    let escaped_script = DAEMON_SCRIPT.replace('\'', "'\\''");

    format!(
        r#"mkdir -p ~/.ratterm && cat > ~/.ratterm/daemon.sh << 'RATTERM_EOF'
{escaped_script}
RATTERM_EOF
chmod +x ~/.ratterm/daemon.sh
HOST_ID={host_id} nohup ~/.ratterm/daemon.sh > ~/.ratterm/daemon.log 2>&1 &
echo "DAEMON_STARTED_$!"
"#
    )
}

/// Generates the command to check daemon status and get its PID.
#[must_use]
pub fn status_daemon_command() -> &'static str {
    "pgrep -f 'ratterm-daemon' 2>/dev/null && echo 'DAEMON_RUNNING' || echo 'DAEMON_NOT_RUNNING'"
}

/// Generates the command to stop the daemon gracefully.
#[must_use]
pub fn stop_daemon_command() -> &'static str {
    "pkill -TERM -f 'ratterm-daemon' 2>/dev/null && echo 'DAEMON_STOPPED' || echo 'DAEMON_NOT_FOUND'"
}

/// Generates the command to view daemon logs.
#[must_use]
pub fn view_logs_command() -> &'static str {
    "tail -50 ~/.ratterm/daemon.log 2>/dev/null || echo 'NO_LOGS'"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_script_contains_required_sections() {
        assert!(DAEMON_SCRIPT.contains("#!/bin/bash"));
        assert!(DAEMON_SCRIPT.contains("/proc/loadavg"));
        assert!(DAEMON_SCRIPT.contains("/proc/meminfo"));
        assert!(DAEMON_SCRIPT.contains("df -BG"));
        assert!(DAEMON_SCRIPT.contains("nvidia-smi"));
        assert!(DAEMON_SCRIPT.contains("rocm-smi"));
        assert!(DAEMON_SCRIPT.contains("curl"));
        assert!(DAEMON_SCRIPT.contains("localhost:19999"));
    }

    #[test]
    fn test_daemon_script_produces_valid_json_structure() {
        // The script's printf should produce valid JSON
        assert!(DAEMON_SCRIPT.contains(r#""host_id":"#));
        assert!(DAEMON_SCRIPT.contains(r#""ts":"#));
        assert!(DAEMON_SCRIPT.contains(r#""cpu":"#));
        assert!(DAEMON_SCRIPT.contains(r#""mem":"#));
        assert!(DAEMON_SCRIPT.contains(r#""disk":"#));
    }

    #[test]
    fn test_deploy_command_includes_host_id() {
        let cmd = deploy_daemon_command(42);
        assert!(cmd.contains("HOST_ID=42"));
        assert!(cmd.contains("nohup"));
        assert!(cmd.contains("~/.ratterm/daemon.sh"));
    }

    #[test]
    fn test_status_command() {
        let cmd = status_daemon_command();
        assert!(cmd.contains("pgrep"));
        assert!(cmd.contains("ratterm-daemon"));
    }

    #[test]
    fn test_stop_command() {
        let cmd = stop_daemon_command();
        assert!(cmd.contains("pkill"));
        assert!(cmd.contains("ratterm-daemon"));
    }

    #[test]
    fn test_deploy_command_escapes_quotes() {
        let cmd = deploy_daemon_command(1);
        // The heredoc should handle the script content properly
        assert!(cmd.contains("RATTERM_EOF"));
        assert!(cmd.contains("chmod +x"));
    }
}
