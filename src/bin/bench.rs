//! Profiling binary: times backtest and walk-forward across all regime datasets.
//!
//! Outputs timing, throughput, memory, and regime classification per dataset.
//! Writes results to out/bench/report.json and out/bench/{date}.json.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, run_backtest_full, BacktestResult, CsvRow};
use arbitragefx::regime::{classify_dataset, RegimeSummary};
use arbitragefx::state::Config;
use arbitragefx::walk_forward::{walk_forward, WalkForwardResult};
use serde::Serialize;

/// Per-dataset profiling result.
#[derive(Debug, Serialize)]
struct DatasetBench {
    name: String,
    candles: usize,
    regime: RegimeSummary,
    backtest_ms: u128,
    walkforward_ms: u128,
    throughput_candles_per_sec: f64,
    peak_rss_kb: Option<u64>,
    backtest_result: BacktestResult,
    walkforward_result: Option<WalkForwardResult>,
}

/// Aggregate bench report.
#[derive(Debug, Serialize)]
struct BenchReport {
    timestamp: String,
    config_hash: String,
    git_sha: String,
    system_info: String,
    datasets: Vec<DatasetBench>,
    total_ms: u128,
    total_candles: usize,
    avg_throughput: f64,
}

/// Read peak RSS from /proc/self/status (Linux only).
fn peak_rss_kb() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().ok();
            }
        }
    }
    None
}

/// Get current git SHA (short).
fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Load CSV rows from file.
fn load_csv(path: &str) -> Vec<CsvRow> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  SKIP {}: {}", path, e);
            return Vec::new();
        }
    };
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if line.to_lowercase().starts_with("ts,") {
            continue;
        }
        if let Ok(r) = parse_csv_line(&line) {
            rows.push(r);
        }
    }
    rows
}

fn main() {
    let cfg = Config::from_env();
    let config_hash = cfg.config_hash();
    let sha = git_sha();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let system_info = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

    // Regime datasets (core 4) plus any extras found
    let core_datasets = vec![
        "data/btc_real_1h.csv",
        "data/btc_bull_1h.csv",
        "data/btc_range_1h.csv",
        "data/btc_bear2_1h.csv",
    ];

    let datasets: Vec<&str> = core_datasets
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    if datasets.is_empty() {
        eprintln!("No datasets found in data/. Nothing to bench.");
        return;
    }

    println!("=== ArbitrageFX Bench ===");
    println!(
        "config_hash={} git={} ts={}",
        &config_hash[..12],
        sha,
        timestamp
    );
    println!();

    let total_start = Instant::now();
    let mut results = Vec::new();
    let mut total_candles = 0usize;

    for path in &datasets {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        print!("  {} ...", name);

        let rows = load_csv(path);
        if rows.is_empty() {
            println!(" no rows, skipping");
            continue;
        }

        let candles = rows.len();
        total_candles += candles;

        // Regime classification
        let regime = classify_dataset(&rows);

        // Backtest timing
        let bt_start = Instant::now();
        let backtest_result = match run_backtest_full(cfg.clone(), &rows) {
            Ok(r) => r,
            Err(e) => {
                println!(" backtest failed: {}", e);
                continue;
            }
        };
        let backtest_ms = bt_start.elapsed().as_millis();

        // Walk-forward timing
        let wf_start = Instant::now();
        let walkforward_result = if rows.len() >= 100 {
            walk_forward(cfg.clone(), &rows, 4, 0.7).ok()
        } else {
            None
        };
        let walkforward_ms = wf_start.elapsed().as_millis();

        // Memory snapshot
        let rss = peak_rss_kb();

        // Throughput: candles processed per second (backtest only)
        let throughput = if backtest_ms > 0 {
            candles as f64 / (backtest_ms as f64 / 1000.0)
        } else {
            candles as f64 * 1000.0 // sub-millisecond
        };

        let best = backtest_result
            .strategies
            .iter()
            .max_by(|a, b| a.equity_pnl.partial_cmp(&b.equity_pnl).unwrap())
            .map(|s| format!("{} ({:+.4})", s.id, s.equity_pnl))
            .unwrap_or_default();

        let survivors = walkforward_result
            .as_ref()
            .map(|wf| {
                wf.summaries
                    .iter()
                    .filter(|s| s.survives_correction)
                    .count()
            })
            .unwrap_or(0);
        let total_strats = walkforward_result
            .as_ref()
            .map(|wf| wf.summaries.len())
            .unwrap_or(0);

        println!(
            " {}c  bt={}ms  wf={}ms  {:.0}c/s  rss={}kb  regime={}  best={}  survivors={}/{}",
            candles,
            backtest_ms,
            walkforward_ms,
            throughput,
            rss.unwrap_or(0),
            regime.dominant_regime,
            best,
            survivors,
            total_strats,
        );

        results.push(DatasetBench {
            name,
            candles,
            regime,
            backtest_ms,
            walkforward_ms,
            throughput_candles_per_sec: throughput,
            peak_rss_kb: rss,
            backtest_result,
            walkforward_result,
        });
    }

    let total_ms = total_start.elapsed().as_millis();
    let avg_throughput = if total_ms > 0 {
        total_candles as f64 / (total_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let report = BenchReport {
        timestamp,
        config_hash,
        git_sha: sha,
        system_info,
        datasets: results,
        total_ms,
        total_candles,
        avg_throughput,
    };

    // Write outputs
    fs::create_dir_all("out/bench").ok();
    let json = serde_json::to_string_pretty(&report).unwrap();

    fs::write("out/bench/report.json", &json).ok();
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    fs::write(format!("out/bench/{}.json", date), &json).ok();

    println!();
    println!("=== Totals ===");
    println!(
        "  datasets={}  candles={}  total={}ms  avg_throughput={:.0}c/s",
        report.datasets.len(),
        total_candles,
        total_ms,
        avg_throughput,
    );
    println!("  out/bench/report.json written");
    println!("  out/bench/{}.json written", date);
}
