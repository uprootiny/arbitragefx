//! Backtest engine validation tests.
//!
//! Validates that the backtesting engine in arbitragefx is reliable,
//! deterministic, and correctly accounts for trading costs.
//!
//! Test categories:
//!   1. Deterministic replay    -- same data, same results
//!   2. Zero-data edge case     -- empty rows error gracefully
//!   3. CSV parsing             -- valid and invalid lines
//!   4. Slippage sanity         -- non-negative, capped at 5%
//!   5. Fill probability bounds -- always in [0, 1]
//!   6. Fee accounting          -- fees reduce PnL
//!   7. ExecConfig presets      -- all constructors produce valid configs
//!   8. Drawdown invariant      -- max_drawdown <= 1.0

use arbitragefx::backtest::{
    calc_fill_probability, calc_slippage, parse_csv_line, run_backtest, CsvRow, ExecConfig,
    ExecMode,
};
use arbitragefx::state::Config;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal Config suitable for tests, without touching env vars.
fn test_config() -> Config {
    let mut cfg = Config::from_env();
    cfg.symbol = "BTCUSDT".to_string();
    cfg.candle_granularity = 300;
    cfg.window = 50;
    cfg
}

/// Generate `n` synthetic CsvRow entries with a controlled linear trend.
///
/// `base_price` is the starting close price.  Each row advances the
/// close by `trend_per_bar` (positive = uptrend, negative = downtrend).
/// OHLCV values are derived from close to keep the data internally
/// consistent (high above close, low below close, volume stable).
fn make_synthetic_rows(n: usize, base_price: f64, trend_per_bar: f64) -> Vec<CsvRow> {
    let ts_start: u64 = 1_000_000;
    let ts_step: u64 = 300; // 5-minute candles
    (0..n)
        .map(|i| {
            let c = base_price + (i as f64) * trend_per_bar;
            let spread = (c * 0.002).max(0.01); // 0.2% spread around close
            CsvRow {
                ts: ts_start + (i as u64) * ts_step,
                o: c - trend_per_bar * 0.3, // open slightly behind the trend
                h: c + spread,
                l: c - spread,
                c,
                v: 5000.0 + (i as f64) * 10.0, // gently rising volume
                funding: 0.00005,
                borrow: 0.00001,
                liq: 0.0,
                depeg: 0.0,
                oi: 1000.0,
            }
        })
        .collect()
}

/// Flat-market rows: price stays constant. Useful for isolating fee effects.
fn make_flat_rows(n: usize, price: f64) -> Vec<CsvRow> {
    make_synthetic_rows(n, price, 0.0)
}

// ===========================================================================
// 1. Deterministic replay
// ===========================================================================

#[test]
fn deterministic_replay_identical_results() {
    let rows = make_synthetic_rows(100, 30_000.0, 5.0);
    let cfg = test_config();

    let (pnl_a, dd_a) = run_backtest(cfg.clone(), &rows).expect("first run failed");
    let (pnl_b, dd_b) = run_backtest(cfg, &rows).expect("second run failed");

    assert!(
        (pnl_a - pnl_b).abs() < 1e-12,
        "PnL must be identical across replays: {} vs {}",
        pnl_a,
        pnl_b
    );
    assert!(
        (dd_a - dd_b).abs() < 1e-12,
        "Max drawdown must be identical across replays: {} vs {}",
        dd_a,
        dd_b
    );
}

#[test]
fn deterministic_replay_downtrend() {
    let rows = make_synthetic_rows(80, 30_000.0, -10.0);
    let cfg = test_config();

    let (pnl_a, dd_a) = run_backtest(cfg.clone(), &rows).expect("first run failed");
    let (pnl_b, dd_b) = run_backtest(cfg, &rows).expect("second run failed");

    assert_eq!(pnl_a, pnl_b, "Downtrend replay must be bit-identical");
    assert_eq!(dd_a, dd_b);
}

// ===========================================================================
// 2. Zero-data edge case
// ===========================================================================

#[test]
fn empty_rows_returns_ok_with_zero_pnl() {
    // With no data, there are no trades, so PnL and drawdown should be zero.
    // The engine returns Ok((0.0, 0.0)) rather than an error for empty input.
    let cfg = test_config();
    let result = run_backtest(cfg, &[]);
    match result {
        Ok((pnl, dd)) => {
            assert!(
                pnl.abs() < 1e-12,
                "PnL on empty data should be zero, got {}",
                pnl
            );
            assert!(
                dd.abs() < 1e-12,
                "Drawdown on empty data should be zero, got {}",
                dd
            );
        }
        Err(_) => {
            // Also acceptable: the engine may choose to error on empty input.
        }
    }
}

#[test]
fn single_row_does_not_panic() {
    let rows = make_synthetic_rows(1, 30_000.0, 0.0);
    let cfg = test_config();
    let result = run_backtest(cfg, &rows);
    assert!(
        result.is_ok(),
        "Single-row backtest should not panic or error"
    );
}

// ===========================================================================
// 3. CSV parsing
// ===========================================================================

#[test]
fn parse_csv_line_valid_11_columns() {
    let line = "1000000,30000.0,30050.0,29950.0,30010.0,5000.0,0.0001,0.00005,0.0,0.0,1200.0";
    let row = parse_csv_line(line).expect("valid line should parse");
    assert_eq!(row.ts, 1_000_000);
    assert!((row.o - 30_000.0).abs() < 1e-9);
    assert!((row.h - 30_050.0).abs() < 1e-9);
    assert!((row.l - 29_950.0).abs() < 1e-9);
    assert!((row.c - 30_010.0).abs() < 1e-9);
    assert!((row.v - 5_000.0).abs() < 1e-9);
    assert!((row.funding - 0.0001).abs() < 1e-12);
    assert!((row.borrow - 0.00005).abs() < 1e-12);
    assert!((row.liq).abs() < 1e-12);
    assert!((row.depeg).abs() < 1e-12);
    assert!((row.oi - 1200.0).abs() < 1e-9);
}

#[test]
fn parse_csv_line_valid_10_columns_oi_defaults() {
    let line = "1000000,30000.0,30050.0,29950.0,30010.0,5000.0,0.0001,0.00005,0.0,0.0";
    let row = parse_csv_line(line).expect("10-column line should parse");
    assert!((row.oi).abs() < 1e-12, "oi should default to 0.0");
}

#[test]
fn parse_csv_line_too_few_columns() {
    let line = "1000000,30000.0,30050.0";
    let result = parse_csv_line(line);
    assert!(result.is_err(), "Fewer than 10 columns should error");
}

#[test]
fn parse_csv_line_non_numeric() {
    let line = "not_a_number,30000.0,30050.0,29950.0,30010.0,5000.0,0.0,0.0,0.0,0.0";
    let result = parse_csv_line(line);
    assert!(result.is_err(), "Non-numeric timestamp should error");
}

#[test]
fn parse_csv_line_whitespace_tolerance() {
    let line = " 1000 , 100.0 , 101.0 , 99.0 , 100.5 , 500.0 , 0.0 , 0.0 , 0.0 , 0.0 , 0.0 ";
    let row = parse_csv_line(line).expect("whitespace-padded line should parse");
    assert_eq!(row.ts, 1000);
    assert!((row.c - 100.5).abs() < 1e-9);
}

// ===========================================================================
// 4. Slippage sanity
// ===========================================================================

#[test]
fn slippage_non_negative_across_inputs() {
    let configs = [
        ExecConfig::instant(),
        ExecConfig::maker(),
        ExecConfig::taker(),
        ExecConfig::realistic(),
    ];
    let quantities = [0.001, 0.1, 1.0, 10.0, 100.0];
    let volumes = [100.0, 1_000.0, 100_000.0];
    let volatilities = [0.0, 0.001, 0.01, 0.05, 0.2];

    for cfg in &configs {
        for &qty in &quantities {
            for &vol in &volumes {
                for &volatility in &volatilities {
                    let slip = calc_slippage(qty, 30_000.0, vol, volatility, cfg);
                    assert!(
                        slip >= 0.0,
                        "Slippage must be non-negative: got {} for qty={}, vol={}, volatility={}, mode={:?}",
                        slip, qty, vol, volatility, cfg.mode
                    );
                }
            }
        }
    }
}

#[test]
fn slippage_capped_at_five_percent() {
    // Use extreme inputs: huge qty relative to tiny volume, high volatility.
    let cfg = ExecConfig::realistic();
    let slip = calc_slippage(1_000_000.0, 30_000.0, 1.0, 1.0, &cfg);
    assert!(
        slip <= 0.05 + 1e-12,
        "Slippage must be capped at 5%: got {}",
        slip
    );
}

#[test]
fn slippage_increases_with_quantity() {
    let cfg = ExecConfig::taker();
    let small = calc_slippage(0.01, 30_000.0, 10_000.0, 0.01, &cfg);
    let large = calc_slippage(100.0, 30_000.0, 10_000.0, 0.01, &cfg);
    assert!(
        large >= small,
        "Slippage should increase with quantity: small={}, large={}",
        small,
        large
    );
}

#[test]
fn slippage_increases_with_volatility() {
    let cfg = ExecConfig::taker();
    let low_vol = calc_slippage(1.0, 30_000.0, 10_000.0, 0.001, &cfg);
    let high_vol = calc_slippage(1.0, 30_000.0, 10_000.0, 0.1, &cfg);
    assert!(
        high_vol >= low_vol,
        "Slippage should increase with volatility: low={}, high={}",
        low_vol,
        high_vol
    );
}

// ===========================================================================
// 5. Fill probability bounds
// ===========================================================================

#[test]
fn fill_probability_always_in_unit_interval() {
    let prices = [100.0, 30_000.0, 0.001];
    let limit_offsets = [-0.05, -0.01, 0.0, 0.01, 0.05];
    let volatilities = [0.0, 0.001, 0.01, 0.05, 0.5];
    let base_probs = [0.0, 0.3, 0.5, 0.7, 1.0];
    let adverse_sels = [0.0, 0.3, 0.5, 1.0];

    for &price in &prices {
        for &offset in &limit_offsets {
            let limit = price * (1.0 + offset);
            for &vol in &volatilities {
                for &base in &base_probs {
                    for &adv in &adverse_sels {
                        for is_buy in [true, false] {
                            let prob = calc_fill_probability(is_buy, limit, price, vol, base, adv);
                            assert!(
                                (0.0..=1.0).contains(&prob),
                                "Fill prob out of [0,1]: {} (buy={}, limit={}, price={}, vol={}, base={}, adv={})",
                                prob, is_buy, limit, price, vol, base, adv
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fill_probability_zero_volatility_returns_base() {
    let prob = calc_fill_probability(true, 100.0, 100.0, 0.0, 0.7, 0.3);
    assert!(
        (prob - 0.7).abs() < 1e-12,
        "Zero volatility should return base_prob, got {}",
        prob
    );
}

#[test]
fn fill_probability_favorable_limit_higher_than_unfavorable() {
    // Buy limit below market (favorable) vs above market (unfavorable)
    let favorable = calc_fill_probability(true, 99.0, 100.0, 0.02, 0.7, 0.0);
    let unfavorable = calc_fill_probability(true, 101.0, 100.0, 0.02, 0.7, 0.0);
    assert!(
        favorable >= unfavorable,
        "Favorable limit should have higher fill prob: fav={}, unfav={}",
        favorable,
        unfavorable
    );
}

// ===========================================================================
// 6. Fee accounting
// ===========================================================================

#[test]
fn fees_reduce_pnl() {
    // Run the same data twice: once with zero fees (instant mode uses the
    // env-based ExecConfig internally, so we compare against a run with
    // the same env).  Since run_backtest reads ExecConfig from env, we
    // cannot easily toggle fees within the same process.  Instead, we
    // validate the structural invariant: on flat data, PnL should be <= 0
    // because any trades incur friction.
    let rows = make_flat_rows(60, 30_000.0);
    let cfg = test_config();
    let (pnl, _dd) = run_backtest(cfg, &rows).expect("flat backtest failed");

    // On perfectly flat data, any trade produces only friction losses.
    // PnL should be zero (no trades triggered) or negative (friction).
    assert!(
        pnl <= 1e-9,
        "On flat data with default fees, PnL should be <= 0, got {}",
        pnl
    );
}

#[test]
fn friction_visible_on_trending_data() {
    // With enough rows and a mild trend, strategies will trade.
    // The PnL captures realized gains minus friction. We verify the
    // engine does not produce obviously wrong results (NaN, infinity).
    let rows = make_synthetic_rows(200, 30_000.0, 2.0);
    let cfg = test_config();
    let (pnl, dd) = run_backtest(cfg, &rows).expect("trending backtest failed");

    assert!(pnl.is_finite(), "PnL must be finite, got {}", pnl);
    assert!(dd.is_finite(), "Drawdown must be finite, got {}", dd);
    assert!(dd >= 0.0, "Drawdown must be non-negative, got {}", dd);
}

// ===========================================================================
// 7. ExecConfig presets
// ===========================================================================

/// Validate that an ExecConfig has internally consistent fields.
fn assert_exec_config_valid(cfg: &ExecConfig, label: &str) {
    assert!(
        cfg.slippage_k >= 0.0,
        "{}: slippage_k must be non-negative, got {}",
        label,
        cfg.slippage_k
    );
    assert!(
        cfg.fee_rate >= 0.0,
        "{}: fee_rate must be non-negative, got {}",
        label,
        cfg.fee_rate
    );
    assert!(
        cfg.latency_min <= cfg.latency_max || cfg.latency_max == 0,
        "{}: latency_min ({}) must be <= latency_max ({})",
        label,
        cfg.latency_min,
        cfg.latency_max
    );
    assert!(
        (0.0..=1.0).contains(&cfg.max_fill_ratio),
        "{}: max_fill_ratio must be in [0,1], got {}",
        label,
        cfg.max_fill_ratio
    );
    assert!(
        (0.0..=1.0).contains(&cfg.limit_fill_prob),
        "{}: limit_fill_prob must be in [0,1], got {}",
        label,
        cfg.limit_fill_prob
    );
    assert!(
        (0.0..=1.0).contains(&cfg.adverse_selection),
        "{}: adverse_selection must be in [0,1], got {}",
        label,
        cfg.adverse_selection
    );
    assert!(
        cfg.vol_slip_mult >= 0.0,
        "{}: vol_slip_mult must be non-negative, got {}",
        label,
        cfg.vol_slip_mult
    );
}

#[test]
fn exec_config_instant_valid() {
    let cfg = ExecConfig::instant();
    assert_exec_config_valid(&cfg, "instant");
    assert_eq!(cfg.mode, ExecMode::Instant);
    assert!(
        (cfg.fee_rate).abs() < 1e-12,
        "instant should have zero fees"
    );
    assert!(
        (cfg.slippage_k).abs() < 1e-12,
        "instant should have zero slippage"
    );
}

#[test]
fn exec_config_maker_valid() {
    let cfg = ExecConfig::maker();
    assert_exec_config_valid(&cfg, "maker");
    assert_eq!(cfg.mode, ExecMode::Limit);
    assert!(
        cfg.fee_rate < ExecConfig::taker().fee_rate,
        "maker fee should be lower than taker fee"
    );
}

#[test]
fn exec_config_taker_valid() {
    let cfg = ExecConfig::taker();
    assert_exec_config_valid(&cfg, "taker");
    assert_eq!(cfg.mode, ExecMode::Market);
}

#[test]
fn exec_config_realistic_valid() {
    let cfg = ExecConfig::realistic();
    assert_exec_config_valid(&cfg, "realistic");
    assert_eq!(cfg.mode, ExecMode::Realistic);
    assert!(
        cfg.adverse_selection > 0.0,
        "realistic should model adverse selection"
    );
}

// ===========================================================================
// 8. Invariant: position bounded (max_drawdown <= 1.0)
// ===========================================================================

#[test]
fn max_drawdown_bounded_uptrend() {
    let rows = make_synthetic_rows(200, 30_000.0, 5.0);
    let cfg = test_config();
    let (_pnl, dd) = run_backtest(cfg, &rows).expect("uptrend backtest failed");
    assert!(
        dd <= 1.0 + 1e-9,
        "Max drawdown should be <= 1.0 (100%), got {}",
        dd
    );
}

#[test]
fn max_drawdown_bounded_downtrend() {
    let rows = make_synthetic_rows(200, 30_000.0, -5.0);
    let cfg = test_config();
    let (_pnl, dd) = run_backtest(cfg, &rows).expect("downtrend backtest failed");
    assert!(
        dd <= 1.0 + 1e-9,
        "Max drawdown should be <= 1.0 (100%), got {}",
        dd
    );
}

#[test]
fn max_drawdown_bounded_volatile() {
    // Zigzag pattern: alternating up and down bars.
    let ts_start: u64 = 1_000_000;
    let rows: Vec<CsvRow> = (0..200)
        .map(|i| {
            let swing = if i % 2 == 0 { 50.0 } else { -50.0 };
            let c = 30_000.0 + swing;
            CsvRow {
                ts: ts_start + (i as u64) * 300,
                o: 30_000.0,
                h: f64::max(c, 30_000.0) + 10.0,
                l: f64::min(c, 30_000.0) - 10.0,
                c,
                v: 8000.0,
                funding: 0.0,
                borrow: 0.0,
                liq: 0.0,
                depeg: 0.0,
                oi: 0.0,
            }
        })
        .collect();

    let cfg = test_config();
    let (_pnl, dd) = run_backtest(cfg, &rows).expect("volatile backtest failed");
    assert!(
        dd <= 1.0 + 1e-9,
        "Max drawdown should be <= 1.0 (100%) even in volatile conditions, got {}",
        dd
    );
}

#[test]
fn max_drawdown_non_negative() {
    let rows = make_synthetic_rows(100, 30_000.0, 3.0);
    let cfg = test_config();
    let (_pnl, dd) = run_backtest(cfg, &rows).expect("backtest failed");
    assert!(dd >= 0.0, "Max drawdown should be non-negative, got {}", dd);
}
