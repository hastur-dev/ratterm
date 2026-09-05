//! End-to-end tests for the metric history: ingest, storage, and what the
//! dashboard actually draws.
//!
//! The unit tests in `src/telemetry` cover the numbers. These cover the path a
//! user takes: samples arrive from a collector, are stored, and appear in the
//! detail view as a chart with a summary. They render through `TestBackend`,
//! so they run on every platform with no terminal.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ratterm::ssh::metrics::{DeviceMetrics, GpuMetrics, GpuType, MetricStatus};
use ratterm::ssh::{SSHCredentials, SSHHostList};
use ratterm::store::{AlertMetric, AlertRule, Comparison, MetricKind};
use ratterm::telemetry::{AlertSettings, HostHistory, Telemetry, offline_for, sparkline};
use ratterm::ui::health_dashboard::{HealthDashboard, HealthDashboardWidget};

/// A plausible online sample, with a distinct CPU reading.
fn sample(host_id: u32, cpu: f32) -> DeviceMetrics {
    let mut metrics = DeviceMetrics::new(host_id);
    metrics.status = MetricStatus::Online;
    metrics.cpu_cores = 8;
    metrics.cpu_usage_percent = cpu;
    metrics.load_avg = (1.0, 1.0, 1.0);
    metrics.mem_total_mb = 16_384;
    metrics.mem_used_mb = 8_192;
    metrics.mem_available_mb = 8_192;
    metrics.disk_total_gb = 500;
    metrics.disk_used_gb = 250;
    // Each sample is a distinct collection, which is how the ingest path
    // tells a new reading from the same one polled again.
    metrics.timestamp = Instant::now();
    metrics
}

/// Ingests a run of samples one minute apart, returning the final timestamp.
fn ingest_series(telemetry: &mut Telemetry, host_id: u32, label: &str, cpus: &[f32]) -> i64 {
    let base = 1_700_000_000;
    let mut last = base;

    for (index, cpu) in cpus.iter().enumerate() {
        let ts = base + (index as i64 * 60);
        telemetry.ingest_at(label, &sample(host_id, *cpu), ts);
        std::thread::sleep(Duration::from_millis(1));
        last = ts;
    }

    last
}

/// A dashboard holding one host, in detail mode, showing the given sample.
fn dashboard_with(host_id: u32, metrics: DeviceMetrics) -> HealthDashboard {
    let mut hosts = SSHHostList::new();
    let id = hosts
        .add_host_with_name("gpu-box".to_string(), 22, "gpu-box".to_string())
        .expect("a host");
    hosts.set_credentials(
        id,
        SSHCredentials::new("me".to_string(), Some("pw".to_string())),
    );

    let mut dashboard = HealthDashboard::new(&hosts);
    for host in dashboard.hosts_mut() {
        host.host_id = host_id;
        host.metrics = metrics.clone();
    }
    dashboard.enter_detail();
    dashboard
}

/// Renders a widget and returns the frame as lines of text.
fn render(widget: HealthDashboardWidget<'_>) -> Vec<String> {
    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).expect("a terminal");
    terminal
        .draw(|frame| frame.render_widget(widget, frame.area()))
        .expect("a frame");

    terminal
        .backend()
        .buffer()
        .content()
        .chunks(100)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn a_stored_series_reaches_the_detail_view_as_a_chart() {
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let last = ingest_series(&mut telemetry, 1, "gpu-box", &[10.0, 40.0, 70.0, 95.0]);

    let history = telemetry.host_history_at(1, Duration::from_secs(3600), 40, last);
    let dashboard = dashboard_with(1, sample(1, 95.0));
    let lines = render(HealthDashboardWidget::new(&dashboard).with_history(&history));
    let frame = lines.join("\n");

    assert!(frame.contains("History"), "no history section:\n{frame}");
    assert!(frame.contains("CPU:"), "no CPU series:\n{frame}");
    assert!(
        frame.contains("min 10%") && frame.contains("max 95%"),
        "the summary is missing or wrong:\n{frame}"
    );
    assert!(
        frame
            .chars()
            .any(|c| ('\u{2581}'..='\u{2588}').contains(&c)),
        "no block characters were drawn:\n{frame}"
    );
}

#[test]
fn a_view_with_no_database_explains_itself_rather_than_showing_an_empty_chart() {
    let telemetry = Telemetry::in_memory_only();
    let history = telemetry.host_history(1, Duration::from_secs(3600), 40);

    let dashboard = dashboard_with(1, sample(1, 50.0));
    let lines = render(HealthDashboardWidget::new(&dashboard).with_history(&history));
    let frame = lines.join("\n");

    assert!(
        frame.contains("metrics_history"),
        "the view should name the setting that turns it on:\n{frame}"
    );
}

#[test]
fn a_host_with_a_database_but_no_samples_says_so() {
    let telemetry = Telemetry::ephemeral().expect("a store");
    let history = telemetry.host_history(7, Duration::from_secs(3600), 40);

    let dashboard = dashboard_with(7, sample(7, 50.0));
    let lines = render(HealthDashboardWidget::new(&dashboard).with_history(&history));
    let frame = lines.join("\n");

    assert!(frame.contains("No samples yet"), "{frame}");
}

#[test]
fn an_offline_host_shows_how_long_it_has_been_gone() {
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let last = ingest_series(&mut telemetry, 3, "rock-5c", &[20.0, 25.0]);

    // The host stops reporting; two hours pass.
    let now = last + 7_200;
    let history = telemetry.host_history_at(3, Duration::from_secs(24 * 3600), 40, now);

    let mut offline = DeviceMetrics::new(3);
    offline.status = MetricStatus::Offline;
    offline.error = Some("connection refused".to_string());

    let dashboard = dashboard_with(3, offline);
    let lines = render(
        HealthDashboardWidget::new(&dashboard)
            .with_history(&history)
            .at(now),
    );
    let frame = lines.join("\n");

    assert!(frame.contains("connection refused"), "{frame}");
    assert!(frame.contains("Last seen: 2h 0m ago"), "{frame}");
}

#[test]
fn a_live_host_is_not_told_it_has_been_offline_for_no_time() {
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let last = ingest_series(&mut telemetry, 4, "cthulhu", &[30.0, 35.0]);
    let history = telemetry.host_history_at(4, Duration::from_secs(3600), 40, last);

    let dashboard = dashboard_with(4, sample(4, 35.0));
    let lines = render(
        HealthDashboardWidget::new(&dashboard)
            .with_history(&history)
            .at(last),
    );
    let frame = lines.join("\n");

    assert!(!frame.contains("Last seen"), "{frame}");
}

#[test]
fn both_collectors_write_one_series_for_a_host() {
    // The SSH poller and the push daemon produce the same DeviceMetrics by
    // different transports. Recording both must not double-count the host.
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let base = 1_700_000_000;

    let ssh_sample = sample(5, 40.0);
    telemetry.ingest_at("cthulhu", &ssh_sample, base);
    std::thread::sleep(Duration::from_millis(1));
    let daemon_sample = sample(5, 60.0);
    telemetry.ingest_at("cthulhu", &daemon_sample, base + 60);

    let summary = telemetry
        .summary(5, base - 60, base + 120)
        .expect("a summary");
    assert_eq!(summary.sample_count, 2, "{summary:?}");

    let stats = summary.stats(MetricKind::CpuPercent).expect("cpu");
    assert!((stats.min - 40.0).abs() < 0.5, "{stats:?}");
    assert!((stats.max - 60.0).abs() < 0.5, "{stats:?}");
}

#[test]
fn polling_the_same_sample_repeatedly_does_not_inflate_the_history() {
    // The dashboard polls on every frame and returns its cached sample until
    // the collector produces a new one.
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let base = 1_700_000_000;
    let cached = sample(6, 55.0);

    for offset in 0..60 {
        telemetry.ingest_at("pi", &cached, base + offset);
    }

    let summary = telemetry
        .summary(6, base - 60, base + 120)
        .expect("a summary");
    assert_eq!(summary.sample_count, 1, "{summary:?}");
}

#[test]
fn an_alert_fires_from_a_configured_threshold() {
    let mut settings = AlertSettings::default();
    settings.apply("alert.cpu", "80");

    let mut telemetry = Telemetry::ephemeral().expect("a store");
    telemetry.set_rules(settings.to_rules());

    let base = 1_700_000_000;
    let quiet = telemetry.ingest_at("gpu-box", &sample(8, 40.0), base);
    assert!(quiet.is_empty(), "a quiet host should not alert: {quiet:?}");

    std::thread::sleep(Duration::from_millis(1));
    let loud = telemetry.ingest_at("gpu-box", &sample(8, 95.0), base + 60);
    assert_eq!(loud.len(), 1, "{loud:?}");
    assert!(loud[0].contains("gpu-box"), "{loud:?}");

    let recorded = telemetry.recent_alerts();
    assert!(!recorded.is_empty(), "the alert should be recorded");
}

#[test]
fn a_temperature_alert_reads_the_gpu_sensor() {
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    telemetry.set_rules(vec![AlertRule::new(
        AlertMetric::TemperatureC,
        Comparison::Above,
        80.0,
    )]);

    let mut hot = sample(9, 30.0);
    hot.gpu = Some(GpuMetrics {
        name: "RTX 5060 Ti".to_string(),
        gpu_type: GpuType::Nvidia,
        usage_percent: 90.0,
        memory_used_mb: 8_192,
        memory_total_mb: 16_384,
        temperature_celsius: Some(91.0),
    });

    let fired = telemetry.ingest_at("gpu-box", &hot, 1_700_000_000);
    assert_eq!(fired.len(), 1, "{fired:?}");
}

#[test]
fn an_offline_sample_is_kept_live_but_not_plotted() {
    let mut telemetry = Telemetry::ephemeral().expect("a store");
    let mut down = DeviceMetrics::new(10);
    down.status = MetricStatus::Offline;
    down.error = Some("no route to host".to_string());

    let fired = telemetry.ingest_at("missing", &down, 1_700_000_000);
    assert!(fired.is_empty(), "an unreachable host cannot breach a rule");

    let live = telemetry.latest(10).expect("the live view keeps it");
    assert_eq!(live.status, MetricStatus::Offline);

    // Nothing to plot: a row of nulls would make "offline since" wrong.
    let history = telemetry.host_history_at(10, Duration::from_secs(3600), 8, 1_700_000_060);
    assert!(history.is_empty(), "{history:?}");
}

#[test]
fn the_history_helpers_agree_with_what_is_rendered() {
    // The widget calls these directly; if they disagree with the assertions
    // above, one of the two is testing the wrong thing.
    let history = HostHistory {
        durable: true,
        window: Duration::from_secs(3600),
        cpu: vec![Some(10.0), None, Some(90.0)],
        memory: vec![Some(50.0), Some(50.0), Some(50.0)],
        summary: None,
        offline_since: Some(1_700_000_000),
    };

    assert!(!history.is_empty());
    assert_eq!(sparkline(&history.cpu).chars().count(), 3);
    assert_eq!(
        offline_for(history.offline_since, 1_700_003_600).as_deref(),
        Some("1h 0m")
    );
}
