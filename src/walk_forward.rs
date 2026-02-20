//! Walk-forward validation and multiple testing correction.
//!
//! Splits data by timestamp into train/test windows, runs backtests on each,
//! and applies Bonferroni correction to significance claims.

use crate::backtest::{run_backtest_full, BacktestResult, CsvRow};
use crate::state::Config;
use anyhow::Result;
use serde::Serialize;

/// Result of a single walk-forward window.
#[derive(Debug, Clone, Serialize)]
pub struct WindowResult {
    pub window_idx: usize,
    pub train_candles: usize,
    pub test_candles: usize,
    pub train_start_ts: u64,
    pub test_start_ts: u64,
    pub test_end_ts: u64,
    pub train: BacktestResult,
    pub test: BacktestResult,
}

/// Per-strategy summary across all walk-forward windows.
#[derive(Debug, Clone, Serialize)]
pub struct StrategySummary {
    pub id: String,
    pub train_mean_pnl: f64,
    pub test_mean_pnl: f64,
    pub overfit_ratio: f64,
    pub test_positive_windows: usize,
    pub total_windows: usize,
    pub test_mean_drawdown: f64,
    /// Raw p-value: fraction of windows where test PnL <= 0
    pub p_value: f64,
    /// Bonferroni-corrected significance (p_value * num_comparisons < alpha)
    pub survives_correction: bool,
}

/// Full walk-forward validation result.
#[derive(Debug, Clone, Serialize)]
pub struct WalkForwardResult {
    pub windows: Vec<WindowResult>,
    pub summaries: Vec<StrategySummary>,
    pub num_strategies: usize,
    pub num_windows: usize,
    pub num_comparisons: usize,
    pub alpha: f64,
    pub correction_method: String,
}

impl WalkForwardResult {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Split rows into train/test at the given fraction (e.g., 0.7 for 70/30).
pub fn train_test_split(rows: &[CsvRow], train_fraction: f64) -> (&[CsvRow], &[CsvRow]) {
    let split_idx = (rows.len() as f64 * train_fraction).round() as usize;
    let split_idx = split_idx.min(rows.len());
    (&rows[..split_idx], &rows[split_idx..])
}

/// Run a single train/test validation.
pub fn validate_split(cfg: Config, rows: &[CsvRow], train_fraction: f64) -> Result<WindowResult> {
    let (train_rows, test_rows) = train_test_split(rows, train_fraction);
    if train_rows.is_empty() || test_rows.is_empty() {
        return Err(anyhow::anyhow!("train or test split is empty"));
    }
    let train = run_backtest_full(cfg.clone(), train_rows)?;
    let test = run_backtest_full(cfg, test_rows)?;
    Ok(WindowResult {
        window_idx: 0,
        train_candles: train_rows.len(),
        test_candles: test_rows.len(),
        train_start_ts: train_rows.first().map(|r| r.ts).unwrap_or(0),
        test_start_ts: test_rows.first().map(|r| r.ts).unwrap_or(0),
        test_end_ts: test_rows.last().map(|r| r.ts).unwrap_or(0),
        train,
        test,
    })
}

/// Run rolling walk-forward validation.
///
/// Divides data into `num_windows` overlapping windows. Each window uses
/// `train_fraction` of its slice for training and the remainder for testing.
/// The windows advance by `step_size = total_rows / (num_windows + 1)` rows.
pub fn walk_forward(
    cfg: Config,
    rows: &[CsvRow],
    num_windows: usize,
    train_fraction: f64,
) -> Result<WalkForwardResult> {
    if rows.len() < 100 {
        return Err(anyhow::anyhow!("too few rows for walk-forward (need >= 100)"));
    }
    let num_windows = num_windows.max(1);
    let window_size = rows.len() * 2 / (num_windows + 1);
    let step = (rows.len() - window_size).max(1) / num_windows.max(1);

    let mut windows = Vec::new();
    for i in 0..num_windows {
        let start = i * step;
        let end = (start + window_size).min(rows.len());
        if end - start < 20 {
            continue;
        }
        let window_rows = &rows[start..end];
        let (train_rows, test_rows) = train_test_split(window_rows, train_fraction);
        if train_rows.len() < 10 || test_rows.len() < 5 {
            continue;
        }
        let train = run_backtest_full(cfg.clone(), train_rows)?;
        let test = run_backtest_full(cfg.clone(), test_rows)?;
        windows.push(WindowResult {
            window_idx: i,
            train_candles: train_rows.len(),
            test_candles: test_rows.len(),
            train_start_ts: train_rows.first().map(|r| r.ts).unwrap_or(0),
            test_start_ts: test_rows.first().map(|r| r.ts).unwrap_or(0),
            test_end_ts: test_rows.last().map(|r| r.ts).unwrap_or(0),
            train,
            test,
        });
    }

    if windows.is_empty() {
        return Err(anyhow::anyhow!("no valid walk-forward windows produced"));
    }

    // Build per-strategy summaries
    let num_strats = windows[0].test.strategies.len();
    let alpha = 0.05;
    let num_comparisons = num_strats * windows.len();

    let mut summaries = Vec::new();
    for s_idx in 0..num_strats {
        let id = windows[0].test.strategies.get(s_idx)
            .map(|s| s.id.clone())
            .unwrap_or_else(|| format!("strategy-{}", s_idx));

        let mut train_pnls = Vec::new();
        let mut test_pnls = Vec::new();
        let mut test_dds = Vec::new();
        let mut test_positive = 0usize;

        for w in &windows {
            if let Some(train_s) = w.train.strategies.get(s_idx) {
                train_pnls.push(train_s.pnl);
            }
            if let Some(test_s) = w.test.strategies.get(s_idx) {
                test_pnls.push(test_s.pnl);
                test_dds.push(test_s.max_drawdown);
                if test_s.pnl > 0.0 {
                    test_positive += 1;
                }
            }
        }

        let n = test_pnls.len().max(1) as f64;
        let train_mean = train_pnls.iter().sum::<f64>() / train_pnls.len().max(1) as f64;
        let test_mean = test_pnls.iter().sum::<f64>() / n;
        let test_mean_dd = test_dds.iter().sum::<f64>() / n;

        // Overfit ratio: how much of train performance survives in test
        let overfit_ratio = if train_mean.abs() > 1e-9 {
            test_mean / train_mean
        } else {
            0.0
        };

        // Raw p-value: fraction of windows where test PnL <= 0
        let p_value = 1.0 - (test_positive as f64 / n);

        // Bonferroni correction
        let corrected_threshold = alpha / num_comparisons as f64;
        let survives = p_value < corrected_threshold;

        summaries.push(StrategySummary {
            id,
            train_mean_pnl: train_mean,
            test_mean_pnl: test_mean,
            overfit_ratio,
            test_positive_windows: test_positive,
            total_windows: test_pnls.len(),
            test_mean_drawdown: test_mean_dd,
            p_value,
            survives_correction: survives,
        });
    }

    Ok(WalkForwardResult {
        num_strategies: num_strats,
        num_windows: windows.len(),
        num_comparisons,
        alpha,
        correction_method: "Bonferroni".to_string(),
        windows,
        summaries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn load_rows(path: &str) -> Vec<CsvRow> {
        let content = std::fs::read_to_string(path).unwrap();
        content.lines()
            .filter(|l| !l.starts_with("ts") && !l.starts_with('#') && !l.is_empty())
            .filter_map(|l| crate::backtest::parse_csv_line(l).ok())
            .collect()
    }

    #[test]
    fn test_train_test_split_proportions() {
        let rows: Vec<CsvRow> = (0..100).map(|i| CsvRow {
            ts: 1000 + i * 300, o: 100.0, h: 101.0, l: 99.0, c: 100.0,
            v: 1000.0, funding: 0.0, borrow: 0.0, liq: 0.0, depeg: 0.0, oi: 0.0,
        }).collect();
        let (train, test) = train_test_split(&rows, 0.7);
        assert_eq!(train.len(), 70);
        assert_eq!(test.len(), 30);
    }

    #[test]
    fn test_train_test_split_edge_cases() {
        let rows: Vec<CsvRow> = (0..10).map(|i| CsvRow {
            ts: 1000 + i * 300, o: 100.0, h: 101.0, l: 99.0, c: 100.0,
            v: 1000.0, funding: 0.0, borrow: 0.0, liq: 0.0, depeg: 0.0, oi: 0.0,
        }).collect();
        let (train, test) = train_test_split(&rows, 1.0);
        assert_eq!(train.len(), 10);
        assert_eq!(test.len(), 0);
        let (train, test) = train_test_split(&rows, 0.0);
        assert_eq!(train.len(), 0);
        assert_eq!(test.len(), 10);
    }

    #[test]
    fn test_validate_split_on_real_data() {
        let csv = "data/btc_real_1h.csv";
        if !Path::new(csv).exists() {
            return;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = validate_split(cfg, &rows, 0.7).unwrap();
        assert!(result.train_candles > 0);
        assert!(result.test_candles > 0);
        assert_eq!(result.train_candles + result.test_candles, rows.len());
        assert_eq!(result.train.strategies.len(), 12);
        assert_eq!(result.test.strategies.len(), 12);
    }

    #[test]
    fn test_walk_forward_on_real_data() {
        let csv = "data/btc_real_1h.csv";
        if !Path::new(csv).exists() {
            return;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = walk_forward(cfg, &rows, 3, 0.7).unwrap();
        assert!(result.windows.len() >= 2, "should have at least 2 windows");
        assert_eq!(result.summaries.len(), 12, "should have 12 strategy summaries");
        assert_eq!(result.correction_method, "Bonferroni");
        // With 12 strategies and 3 windows, num_comparisons should be 36
        assert!(result.num_comparisons > 0);
        for s in &result.summaries {
            assert!(s.total_windows > 0);
            assert!(s.p_value >= 0.0 && s.p_value <= 1.0);
            // Overfit ratio: test/train. Could be negative, but should be finite.
            assert!(s.overfit_ratio.is_finite(), "{} overfit ratio not finite", s.id);
        }
    }

    #[test]
    fn test_walk_forward_serializes() {
        let csv = "data/btc_real_1h.csv";
        if !Path::new(csv).exists() {
            return;
        }
        let rows = load_rows(csv);
        let cfg = Config::from_env();
        let result = walk_forward(cfg, &rows, 3, 0.7).unwrap();
        let json = result.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        assert!(parsed["summaries"].is_array());
        assert!(parsed["correction_method"].as_str() == Some("Bonferroni"));
    }
}
