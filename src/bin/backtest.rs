use std::fs::File;
use std::io::{BufRead, BufReader};

use arbitragefx::backtest::{parse_csv_line, run_backtest};
use arbitragefx::data::analyze_csv;
use arbitragefx::regime::classify_dataset;
use arbitragefx::state::Config;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "data.csv".to_string());
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("failed to open {}: {}", path, err);
            return;
        }
    };
    if std::env::var("VALIDATE_DATA").as_deref() == Ok("1") {
        let interval_secs = std::env::var("DATA_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let ttl_secs = std::env::var("DATA_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match analyze_csv(path.as_ref(), interval_secs, ttl_secs, now_ts) {
            Ok((manifest, report)) => {
                eprintln!(
                    "data_quality rows={} bad_rows={} gaps={} stale={}",
                    report.rows, report.bad_rows, report.gaps, report.stale
                );
                if !manifest.warnings.is_empty() {
                    eprintln!("data_warnings: {:?}", manifest.warnings);
                }
            }
            Err(err) => {
                eprintln!("data_quality_check_failed: {}", err);
            }
        }
    }
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if line.to_lowercase().starts_with("ts,") {
            continue;
        }
        match parse_csv_line(&line) {
            Ok(r) => rows.push(r),
            Err(err) => eprintln!("bad row: {}", err),
        }
    }
    if rows.is_empty() {
        eprintln!("no rows parsed");
        return;
    }
    // Regime classification
    let regime = classify_dataset(&rows);
    println!("regime={} trend={} narrative_score={:.3} price_change={:.1}%",
        regime.dominant_regime, regime.price_trend, regime.mean_narrative_score, regime.price_change_pct);
    println!("regime_breakdown grounded={:.1}% uncertain={:.1}% narrative={:.1}% reflexive={:.1}%",
        regime.grounded_frac * 100.0, regime.uncertain_frac * 100.0,
        regime.narrative_frac * 100.0, regime.reflexive_frac * 100.0);

    let cfg = Config::from_env();
    match run_backtest(cfg, &rows) {
        Ok((pnl, dd)) => println!("pnl_total={:.4} max_drawdown={:.4}", pnl, dd),
        Err(err) => eprintln!("backtest failed: {}", err),
    }
}
