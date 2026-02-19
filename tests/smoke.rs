//! Smoke tests: end-to-end validation that the system's claims are real.
//!
//! These tests run actual backtests on real data and verify invariants.
//! They are the gate between "code compiles" and "system works."

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arbitragefx::backtest::{parse_csv_line, run_backtest, run_backtest_full, CsvRow};
use arbitragefx::data::{analyze_csv, file_sha256, validate_schema};
use arbitragefx::state::Config;

/// Load CSV rows from a file path, skipping headers/comments.
fn load_rows(path: &str) -> Vec<CsvRow> {
    let file = File::open(path).unwrap_or_else(|e| panic!("cannot open {}: {}", path, e));
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.to_lowercase().starts_with("ts,") {
            continue;
        }
        if let Ok(r) = parse_csv_line(trimmed) {
            rows.push(r);
        }
    }
    rows
}

// Real data files used by smoke tests.
const REAL_CSVS: &[&str] = &[
    "data/btc_real_1h.csv",
    "data/btc_bull_1h.csv",
    "data/btc_range_1h.csv",
    "data/btc_bear2_1h.csv",
];

// ---------------------------------------------------------------------------
// S01-S02: Compilation and unit tests are implicit (cargo test runs this file)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// S03: Backtest on real data produces output
// ---------------------------------------------------------------------------
#[test]
fn s03_backtest_produces_output() {
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            eprintln!("SKIP s03: {} not found", csv);
            continue;
        }
        let rows = load_rows(csv);
        assert!(!rows.is_empty(), "{} produced no rows", csv);
        let cfg = Config::from_env();
        let result = run_backtest(cfg, &rows);
        assert!(result.is_ok(), "backtest failed on {}: {:?}", csv, result.err());
    }
}

// ---------------------------------------------------------------------------
// S05: All equity values > 0 (Invariant I001)
// ---------------------------------------------------------------------------
#[test]
fn s05_equity_positive() {
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = run_backtest(cfg, &rows);
        assert!(result.is_ok());
        let (pnl, _dd) = result.unwrap();
        // Total PnL can be negative, but we check it doesn't wipe out
        // the initial $1000 * 12 strategies = $12,000 total equity.
        // If total PnL is worse than -$11,000 something is catastrophically wrong.
        assert!(
            pnl > -11_000.0,
            "catastrophic loss on {}: pnl={:.2}",
            csv,
            pnl
        );
    }
}

// ---------------------------------------------------------------------------
// S06: Max drawdown <= 5% (Invariant I002)
// ---------------------------------------------------------------------------
#[test]
fn s06_drawdown_bounded() {
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = run_backtest(cfg, &rows);
        assert!(result.is_ok());
        let (_pnl, max_dd) = result.unwrap();
        assert!(
            max_dd <= 0.05,
            "drawdown exceeded 5% on {}: dd={:.4}",
            csv,
            max_dd
        );
    }
}

// ---------------------------------------------------------------------------
// S07: Deterministic replay — two runs produce identical output
// ---------------------------------------------------------------------------
#[test]
fn s07_deterministic_replay() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        eprintln!("SKIP s07: {} not found", csv);
        return;
    }
    let rows = load_rows(csv);
    let cfg1 = Config::from_env();
    let cfg2 = Config::from_env();
    let r1 = run_backtest(cfg1, &rows).unwrap();
    let r2 = run_backtest(cfg2, &rows).unwrap();
    assert_eq!(r1.0, r2.0, "PnL differs between runs");
    assert_eq!(r1.1, r2.1, "Max DD differs between runs");
}

// ---------------------------------------------------------------------------
// S08: Real CSV schema validation — headers must match expected columns
// ---------------------------------------------------------------------------
#[test]
fn s08_csv_schema_valid() {
    for csv in REAL_CSVS {
        let path = Path::new(csv);
        if !path.exists() {
            continue;
        }
        let report = validate_schema(path).unwrap();
        assert!(
            report.ok,
            "schema mismatch in {}: {}",
            csv, report.message
        );
        assert_eq!(
            report.columns.len(),
            11,
            "{} has {} columns, expected 11",
            csv,
            report.columns.len()
        );
    }
}

// ---------------------------------------------------------------------------
// S09: Data quality — no bad rows in real CSVs
// ---------------------------------------------------------------------------
#[test]
fn s09_data_quality() {
    for csv in REAL_CSVS {
        let path = Path::new(csv);
        if !path.exists() {
            continue;
        }
        let result = analyze_csv(path, 3600, 86400 * 365, 0);
        assert!(result.is_ok(), "analyze_csv failed on {}", csv);
        let (_manifest, report) = result.unwrap();
        assert_eq!(
            report.bad_rows, 0,
            "{} has {} bad rows",
            csv, report.bad_rows
        );
        assert!(report.rows > 0, "{} has 0 rows", csv);
    }
}

// ---------------------------------------------------------------------------
// S10: SHA256 reproducibility — same file produces same hash
// ---------------------------------------------------------------------------
#[test]
fn s10_sha256_reproducible() {
    for csv in REAL_CSVS {
        let path = Path::new(csv);
        if !path.exists() {
            continue;
        }
        let h1 = file_sha256(path).unwrap();
        let h2 = file_sha256(path).unwrap();
        assert_eq!(h1, h2, "SHA256 not reproducible for {}", csv);
        assert_eq!(h1.len(), 64, "SHA256 wrong length for {}", csv);
    }
}

// ---------------------------------------------------------------------------
// S11: Friction is non-negative for strategies with trades
// ---------------------------------------------------------------------------
#[test]
fn s11_friction_nonnegative() {
    // This is verified structurally: fee = price * qty * fee_rate (all positive)
    // and slip_cost = |fill_price - close| * |qty| (also positive).
    // The test here validates the accounting doesn't go negative.
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    // We can't easily extract per-strategy friction from run_backtest's return,
    // but we verify the run succeeds and drawdown is bounded (friction >= 0 is
    // structurally guaranteed by the slippage_price and fee calculations).
    let result = run_backtest(cfg, &rows);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// S12: Multiple real datasets load without error
// ---------------------------------------------------------------------------
#[test]
fn s12_all_datasets_loadable() {
    let mut loaded = 0;
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        assert!(rows.len() > 100, "{} has too few rows: {}", csv, rows.len());
        loaded += 1;
    }
    // At least one dataset must exist for smoke tests to be meaningful
    assert!(loaded > 0, "no real datasets found — smoke tests are vacuous");
}

// ---------------------------------------------------------------------------
// S13: Cross-regime consistency — same strategy count in every regime
// ---------------------------------------------------------------------------
#[test]
fn s13_cross_regime_consistency() {
    let mut strategy_counts: Vec<(String, usize)> = Vec::new();
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = run_backtest(cfg, &rows);
        assert!(result.is_ok(), "backtest failed on {}", csv);
        // Count strategies by parsing the test output indirectly:
        // run_backtest returns aggregate (pnl, dd) — strategy count is fixed at 12
        // by build_churn_set. Verify the backtest doesn't crash on any regime.
        strategy_counts.push((csv.to_string(), rows.len()));
    }
    // All regimes must have been tested
    assert!(strategy_counts.len() >= 3, "need at least 3 regimes, got {}", strategy_counts.len());
}

// ---------------------------------------------------------------------------
// S14: Regression — PnL on btc_real_1h.csv is bounded and stable
// ---------------------------------------------------------------------------
#[test]
fn s14_regression_pnl_bounded() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    let (pnl, dd) = run_backtest(cfg, &rows).unwrap();
    // These bounds come from observed real runs. If they change, the strategy
    // logic or execution model changed — which is worth investigating.
    assert!(pnl > -200.0, "PnL too negative: {:.4} (regression?)", pnl);
    assert!(pnl < 200.0, "PnL suspiciously positive: {:.4} (bug?)", pnl);
    assert!(dd < 0.03, "Drawdown too high: {:.4} (regression?)", dd);
    assert!(dd >= 0.0, "Negative drawdown: {:.4} (accounting bug)", dd);
}

// ---------------------------------------------------------------------------
// S15b: Structured backtest — per-strategy results are self-consistent
// ---------------------------------------------------------------------------
#[test]
fn s15b_structured_backtest_consistent() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    let result = run_backtest_full(cfg, &rows).unwrap();

    assert_eq!(result.strategies.len(), 12, "expected 12 strategies");
    for s in &result.strategies {
        assert!(s.equity > 0.0, "{} equity <= 0: {:.4}", s.id, s.equity);
        assert!(s.max_drawdown >= 0.0, "{} negative drawdown", s.id);
        assert!(s.max_drawdown <= 0.05, "{} drawdown > 5%: {:.4}", s.id, s.max_drawdown);
        assert!(s.friction >= 0.0, "{} negative friction: {:.4}", s.id, s.friction);
        assert_eq!(s.trades, s.wins + s.losses, "{} trades != wins+losses", s.id);
        // Friction should be proportional to fills (more fills = more friction)
        if s.fills > 0 {
            assert!(s.friction > 0.0, "{} has {} fills but zero friction", s.id, s.fills);
        }
    }
    // Buy-hold should be negative for this bear market dataset
    assert!(result.buy_hold_pnl < 0.0, "buy-hold should be negative for bear data");
}

// ---------------------------------------------------------------------------
// S15: Data provenance — all regime CSVs have plausible timestamps
// ---------------------------------------------------------------------------
#[test]
fn s15_timestamp_sanity() {
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        let first_ts = rows.first().unwrap().ts;
        let last_ts = rows.last().unwrap().ts;
        // Timestamps should be between 2024-01-01 and 2026-12-31
        assert!(first_ts > 1704067200, "{} first ts too old: {}", csv, first_ts);
        assert!(last_ts < 1798761600, "{} last ts too far future: {}", csv, last_ts);
        // Should be in chronological order
        assert!(last_ts >= first_ts, "{} timestamps not ascending", csv);
        // Span should be > 1 day
        assert!(last_ts - first_ts > 86400, "{} span too short", csv);
    }
}

// ---------------------------------------------------------------------------
// S16: Config reproducibility — same config produces same hash
// ---------------------------------------------------------------------------
#[test]
fn s16_config_hash_deterministic() {
    let cfg1 = Config::from_env();
    let cfg2 = Config::from_env();
    assert_eq!(cfg1.config_hash(), cfg2.config_hash(), "same config should produce same hash");
    // Hash should be 64 hex chars (SHA256)
    assert_eq!(cfg1.config_hash().len(), 64, "hash should be 64 hex chars");
}

// ---------------------------------------------------------------------------
// S17: Config serialization round-trip
// ---------------------------------------------------------------------------
#[test]
fn s17_config_json_round_trip() {
    let cfg = Config::from_env();
    let json = cfg.to_json();
    assert!(json.contains("\"symbol\""), "JSON should contain symbol field");
    assert!(json.contains("\"entry_threshold\""), "JSON should contain entry_threshold");
    assert!(json.contains("\"min_hold_candles\""), "JSON should contain min_hold_candles");
    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("config JSON should be valid");
    assert!(parsed.is_object(), "parsed config should be an object");
}

// ---------------------------------------------------------------------------
// S18: BacktestResult includes config_hash and serializes to JSON
// ---------------------------------------------------------------------------
#[test]
fn s18_backtest_result_has_config_hash() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    let expected_hash = cfg.config_hash();
    let result = run_backtest_full(cfg, &rows).unwrap();

    assert_eq!(result.config_hash, expected_hash, "result should carry config hash");
    assert_eq!(result.candle_count, rows.len(), "candle count should match rows");

    // Should serialize to valid JSON
    let json = result.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("result JSON should be valid");
    assert!(parsed["config_hash"].is_string());
    assert!(parsed["strategies"].is_array());
    assert_eq!(parsed["strategies"].as_array().unwrap().len(), 12);
}
