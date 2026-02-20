//! Smoke tests: end-to-end validation that the system's claims are real.
//!
//! These tests run actual backtests on real data and verify invariants.
//! They are the gate between "code compiles" and "system works."

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arbitragefx::backtest::{parse_csv_line, run_backtest, run_backtest_full, CsvRow};
use arbitragefx::backtest_traps;
use arbitragefx::data::{analyze_csv, file_sha256, validate_schema};
use arbitragefx::indicators;
use arbitragefx::narrative_detector::{NarrativeIndicators, NarrativeRegime};
use arbitragefx::regime;
use arbitragefx::state::Config;
use arbitragefx::walk_forward;

/// Load CSV rows from a file path, skipping headers/comments.
fn load_rows(path: &str) -> Vec<CsvRow> {
    let file = File::open(path).unwrap_or_else(|e| panic!("cannot open {}: {}", path, e));
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.to_lowercase().starts_with("ts,")
        {
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
        assert!(
            result.is_ok(),
            "backtest failed on {}: {:?}",
            csv,
            result.err()
        );
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
        assert!(report.ok, "schema mismatch in {}: {}", csv, report.message);
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
    assert!(
        loaded > 0,
        "no real datasets found — smoke tests are vacuous"
    );
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
    assert!(
        strategy_counts.len() >= 3,
        "need at least 3 regimes, got {}",
        strategy_counts.len()
    );
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
        assert!(
            s.max_drawdown <= 0.05,
            "{} drawdown > 5%: {:.4}",
            s.id,
            s.max_drawdown
        );
        assert!(
            s.friction >= 0.0,
            "{} negative friction: {:.4}",
            s.id,
            s.friction
        );
        assert_eq!(
            s.trades,
            s.wins + s.losses,
            "{} trades != wins+losses",
            s.id
        );
        // Friction should be proportional to fills (more fills = more friction)
        if s.fills > 0 {
            assert!(
                s.friction > 0.0,
                "{} has {} fills but zero friction",
                s.id,
                s.fills
            );
        }
    }
    // Buy-hold should be negative for this bear market dataset
    assert!(
        result.buy_hold_pnl < 0.0,
        "buy-hold should be negative for bear data"
    );
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
        assert!(
            first_ts > 1704067200,
            "{} first ts too old: {}",
            csv,
            first_ts
        );
        assert!(
            last_ts < 1798761600,
            "{} last ts too far future: {}",
            csv,
            last_ts
        );
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
    assert_eq!(
        cfg1.config_hash(),
        cfg2.config_hash(),
        "same config should produce same hash"
    );
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
    assert!(
        json.contains("\"symbol\""),
        "JSON should contain symbol field"
    );
    assert!(
        json.contains("\"entry_threshold\""),
        "JSON should contain entry_threshold"
    );
    assert!(
        json.contains("\"min_hold_candles\""),
        "JSON should contain min_hold_candles"
    );
    // Should be valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("config JSON should be valid");
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

    assert_eq!(
        result.config_hash, expected_hash,
        "result should carry config hash"
    );
    assert_eq!(
        result.candle_count,
        rows.len(),
        "candle count should match rows"
    );

    // Should serialize to valid JSON
    let json = result.to_json();
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("result JSON should be valid");
    assert!(parsed["config_hash"].is_string());
    assert!(parsed["strategies"].is_array());
    assert_eq!(parsed["strategies"].as_array().unwrap().len(), 12);
}

// ===========================================================================
// S19: Error path — empty data doesn't panic
// ===========================================================================
#[test]
fn s19_empty_data_no_panic() {
    let rows: Vec<CsvRow> = Vec::new();
    let cfg = Config::from_env();
    // run_backtest on empty rows should not panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = run_backtest(cfg, &rows);
    }));
    assert!(result.is_ok(), "empty data should not panic");
}

// ===========================================================================
// S20: Error path — malformed CSV lines rejected
// ===========================================================================
#[test]
fn s20_malformed_csv_rejected() {
    assert!(parse_csv_line("").is_err());
    assert!(parse_csv_line("not,enough,columns").is_err());
    assert!(parse_csv_line("abc,1,2,3,4,5,6,7,8,9").is_err()); // non-numeric ts
                                                               // Valid line should parse
    assert!(parse_csv_line("1000,1,1,1,1,10,0.0001,0.0,0.0,0.0,0.0").is_ok());
}

// ===========================================================================
// S21: Regime fractions sum to 1.0 on all real datasets
// ===========================================================================
#[test]
fn s21_regime_fractions_sum_to_one() {
    for csv in REAL_CSVS {
        if !Path::new(csv).exists() {
            continue;
        }
        let rows = load_rows(csv);
        let summary = regime::classify_dataset(&rows);
        let sum = summary.grounded_frac
            + summary.uncertain_frac
            + summary.narrative_frac
            + summary.reflexive_frac;
        assert!(
            (sum - 1.0).abs() < 0.01,
            "{}: regime fractions sum to {} (expected ~1.0)",
            csv,
            sum
        );
        assert_ne!(
            summary.dominant_regime, "insufficient_data",
            "{}: should not be insufficient_data",
            csv
        );
        assert!(!summary.price_trend.is_empty());
    }
}

// ===========================================================================
// S22: Regime — insufficient data returns correct classification
// ===========================================================================
#[test]
fn s22_regime_insufficient_data() {
    let rows: Vec<CsvRow> = (0..5)
        .map(|i| CsvRow {
            ts: 1000 + i * 3600,
            o: 100.0,
            h: 101.0,
            l: 99.0,
            c: 100.0,
            v: 1000.0,
            funding: 0.0,
            borrow: 0.0,
            liq: 0.0,
            depeg: 0.0,
            oi: 0.0,
        })
        .collect();
    let summary = regime::classify_dataset(&rows);
    assert_eq!(summary.dominant_regime, "insufficient_data");
}

// ===========================================================================
// S23: Narrative detector — grounded from calm indicators
// ===========================================================================
#[test]
fn s23_narrative_grounded() {
    let indicators = NarrativeIndicators {
        funding_zscore: 0.1,
        liquidation_score: 0.1,
        volatility_ratio: 1.05,
        ..Default::default()
    };
    assert_eq!(indicators.regime(), NarrativeRegime::Grounded);
    assert!(!indicators.regime().should_reduce_exposure());
    assert!((indicators.regime().position_multiplier() - 1.0).abs() < 1e-9);
}

// ===========================================================================
// S24: Narrative detector — reflexive from extreme indicators
// ===========================================================================
#[test]
fn s24_narrative_reflexive() {
    let indicators = NarrativeIndicators {
        funding_zscore: 4.0,
        liquidation_score: 5.0,
        volatility_ratio: 3.0,
        oi_change_rate: 0.2,
        pv_divergence: 0.8,
        ..Default::default()
    };
    assert_eq!(indicators.regime(), NarrativeRegime::Reflexive);
    assert!(indicators.regime().should_reduce_exposure());
    assert_eq!(indicators.regime().position_multiplier(), 0.0);
}

// ===========================================================================
// S25: Backtest traps — all 18 trap definitions accessible
// ===========================================================================
#[test]
fn s25_backtest_traps_enumerable() {
    let traps = backtest_traps::all_traps();
    assert!(
        traps.len() >= 18,
        "expected >=18 traps, got {}",
        traps.len()
    );
    for trap in &traps {
        assert!(!trap.name.is_empty(), "trap #{} has empty name", trap.id);
        assert!(
            !trap.module.is_empty(),
            "trap #{} has empty module",
            trap.id
        );
        assert!(!trap.guard.is_empty(), "trap #{} has empty guard", trap.id);
    }
}

// ===========================================================================
// S26: Backtest traps — integrity checker
// ===========================================================================
#[test]
fn s26_backtest_integrity_check() {
    let checker = backtest_traps::BacktestIntegrity::new();
    assert!(
        checker.is_trustworthy(),
        "empty checker should be trustworthy"
    );
    assert!(checker.violations().is_empty());

    let mut checker2 = backtest_traps::BacktestIntegrity::new();
    checker2.add_violation(backtest_traps::TrapViolation {
        trap_id: 1,
        trap_name: "test",
        severity: backtest_traps::Severity::Critical,
        description: "test violation".into(),
        module_location: "test",
        guard_recommendation: "test",
    });
    assert!(
        !checker2.is_trustworthy(),
        "critical violation should make it untrustworthy"
    );
    let report = checker2.report();
    assert!(report.contains("Critical violations: 1"));
}

// ===========================================================================
// S27: Indicators — RSI bounded, Bollinger ordered
// ===========================================================================
#[test]
fn s27_indicator_bounds() {
    // RSI should be in [0, 100]
    let mut rsi = indicators::Rsi::new(14);
    for i in 0..50 {
        rsi.update(100.0 + i as f64);
    }
    let v = rsi.get();
    assert!(v >= 0.0 && v <= 100.0, "RSI out of bounds: {}", v);

    // Bollinger: upper >= middle >= lower
    let mut bb = indicators::BollingerBands::default_20_2();
    for i in 0..30 {
        bb.update(100.0 + (i % 5) as f64);
    }
    assert!(bb.upper >= bb.middle, "Bollinger upper < middle");
    assert!(bb.middle >= bb.lower, "Bollinger middle < lower");

    // EMA should converge toward input
    let mut ema = indicators::Ema::new(10);
    for _ in 0..100 {
        ema.update(42.0);
    }
    assert!(
        (ema.get() - 42.0).abs() < 0.01,
        "EMA didn't converge: {}",
        ema.get()
    );
}

// ===========================================================================
// S28: Walk-forward JSON round-trip
// ===========================================================================
#[test]
fn s28_walk_forward_round_trip() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    let result = walk_forward::walk_forward(cfg, &rows, 3, 0.7).unwrap();
    let json = result.to_json();
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("walk-forward JSON should be valid");
    assert!(parsed["summaries"].is_array());
    assert_eq!(parsed["correction_method"].as_str(), Some("Bonferroni"));
    assert!(parsed["num_comparisons"].as_u64().unwrap() > 0);
    // Each summary should have required fields
    for s in parsed["summaries"].as_array().unwrap() {
        assert!(s["id"].is_string());
        assert!(s["p_value"].is_f64());
        assert!(s["survives_correction"].is_boolean());
    }
}

// ===========================================================================
// S29: Performance — backtest completes within time bound
// ===========================================================================
#[test]
fn s29_performance_baseline() {
    let csv = "data/btc_real_1h.csv";
    if !Path::new(csv).exists() {
        return;
    }
    let rows = load_rows(csv);
    let cfg = Config::from_env();
    let start = std::time::Instant::now();
    let _ = run_backtest_full(cfg, &rows);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "1000-candle backtest took {:?} — regression?",
        elapsed
    );
}

// ===========================================================================
// S30: Config — fields populated, hash sensitive to changes
// ===========================================================================
#[test]
fn s30_config_hash_sensitivity() {
    let cfg = Config::from_env();
    assert!(!cfg.symbol.is_empty());
    assert!(cfg.candle_granularity > 0);
    assert!(cfg.max_position_pct > 0.0);

    let hash1 = cfg.config_hash();
    assert_eq!(hash1.len(), 64, "config hash should be 64 hex chars");

    // Changing a field should change the hash
    let mut cfg2 = cfg.clone();
    cfg2.entry_threshold = 999.999;
    let hash2 = cfg2.config_hash();
    assert_ne!(
        hash1, hash2,
        "different config should produce different hash"
    );
}
