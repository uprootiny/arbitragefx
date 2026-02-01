//! Ethically-aligned backtesting framework.
//!
//! ## Core Principle
//!
//! A backtest is a **falsification engine for assumptions**, not proof of profit.
//!
//! ## Alignment with Noble Eightfold Path
//!
//! | Principle | Backtesting Application |
//! |-----------|------------------------|
//! | Right View | Preserve uncertainty honestly |
//! | Right Action | Full audit trail, no cherry-picking |
//! | Right Livelihood | Test failures, not fantasies |
//!
//! ## What This Module Provides
//!
//! 1. **Stress injection**: Adversarial conditions to find breaking points
//! 2. **Walk-forward validation**: No peeking at future data
//! 3. **Experiment logging**: Full reproducibility
//! 4. **Failure metrics**: Emphasize worst-case over average

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Experiment configuration for reproducibility (Right Action)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    /// Unique experiment ID
    pub id: String,
    /// Random seed for reproducibility
    pub seed: u64,
    /// Strategy parameters
    pub params: HashMap<String, f64>,
    /// Data range
    pub start_ts: u64,
    pub end_ts: u64,
    /// Git commit hash (if available)
    pub code_version: Option<String>,
    /// Timestamp of experiment
    pub run_ts: u64,
}

/// Comprehensive backtest results emphasizing failure modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    pub config: ExperimentConfig,

    // === Standard metrics ===
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub total_trades: u32,

    // === FAILURE METRICS (Right View: see reality) ===
    /// Maximum drawdown - how badly can this fail?
    pub max_drawdown_pct: f64,
    /// Longest drawdown duration in periods
    pub max_drawdown_duration: u64,
    /// Worst single trade loss
    pub worst_trade_pct: f64,
    /// Consecutive losing trades (max)
    pub max_consecutive_losses: u32,
    /// 95th percentile of losses (tail risk)
    pub loss_95th_percentile: f64,
    /// 99th percentile of losses (extreme tail)
    pub loss_99th_percentile: f64,

    // === REGIME ANALYSIS (Right View: impermanence) ===
    /// Performance in high-volatility regimes
    pub high_vol_return: f64,
    /// Performance in low-volatility regimes
    pub low_vol_return: f64,
    /// Performance in trending markets
    pub trending_return: f64,
    /// Performance in mean-reverting markets
    pub ranging_return: f64,

    // === STRESS TEST RESULTS ===
    pub stress_results: Vec<StressTestResult>,

    // === INTEGRITY CHECKS (Right Action: no deception) ===
    /// Were there any lookahead violations detected?
    pub lookahead_violations: u32,
    /// Data gaps encountered
    pub data_gaps: u32,
    /// Hash of final state for replay validation
    pub final_state_hash: u64,
}

impl BacktestResults {
    /// The "reality score" - emphasizes failure modes over success
    ///
    /// A high score means the system fails gracefully.
    /// This is more valuable than high returns.
    pub fn reality_score(&self) -> f64 {
        // Penalize large drawdowns heavily
        let dd_penalty = (self.max_drawdown_pct * 3.0).min(1.0);

        // Penalize tail risk
        let tail_penalty = (self.loss_99th_percentile.abs() * 10.0).min(1.0);

        // Penalize regime sensitivity (want consistent across regimes)
        let returns = [self.high_vol_return, self.low_vol_return,
                       self.trending_return, self.ranging_return];
        let mean_return = returns.iter().sum::<f64>() / 4.0;
        let regime_variance: f64 = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / 4.0;
        let regime_penalty = (regime_variance.sqrt() * 5.0).min(1.0);

        // Penalize stress test failures
        let stress_failures = self.stress_results.iter()
            .filter(|s| s.survived == false)
            .count() as f64;
        let stress_penalty = stress_failures / self.stress_results.len().max(1) as f64;

        // Base score from returns (but capped to prevent chasing)
        let return_score = (self.total_return_pct * 0.1).min(0.3).max(-0.3);

        // Reality score: high = robust, low = fragile
        1.0 - dd_penalty * 0.3 - tail_penalty * 0.2 - regime_penalty * 0.2 - stress_penalty * 0.3 + return_score
    }

    /// Summary judgment: is this system aligned or dangerous?
    pub fn alignment_assessment(&self) -> AlignmentAssessment {
        let score = self.reality_score();

        if self.lookahead_violations > 0 {
            return AlignmentAssessment::Corrupted {
                reason: "Lookahead violations detected - results are invalid".to_string(),
            };
        }

        if self.max_drawdown_pct > 0.5 {
            return AlignmentAssessment::Dangerous {
                reason: format!("Max drawdown {:.1}% exceeds 50% - ruin risk", self.max_drawdown_pct * 100.0),
            };
        }

        let stress_survival_rate = self.stress_results.iter()
            .filter(|s| s.survived)
            .count() as f64 / self.stress_results.len().max(1) as f64;

        if stress_survival_rate < 0.5 {
            return AlignmentAssessment::Fragile {
                reason: format!("Only {:.0}% stress survival - system is brittle", stress_survival_rate * 100.0),
            };
        }

        if score > 0.6 {
            AlignmentAssessment::Aligned {
                confidence: score,
            }
        } else if score > 0.3 {
            AlignmentAssessment::Uncertain {
                concerns: vec![
                    format!("Reality score {:.2} is marginal", score),
                ],
            }
        } else {
            AlignmentAssessment::Misaligned {
                reason: format!("Reality score {:.2} indicates delusional expectations", score),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlignmentAssessment {
    /// System appears robust and honest
    Aligned { confidence: f64 },
    /// Results are questionable
    Uncertain { concerns: Vec<String> },
    /// System shows signs of overfitting or fragility
    Misaligned { reason: String },
    /// System is brittle under stress
    Fragile { reason: String },
    /// System will likely cause harm
    Dangerous { reason: String },
    /// Results cannot be trusted
    Corrupted { reason: String },
}

/// Result of a single stress test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    pub name: String,
    pub description: String,
    /// Did the system survive without catastrophic loss?
    pub survived: bool,
    /// Drawdown during stress
    pub stress_drawdown_pct: f64,
    /// Did risk guards activate appropriately?
    pub guards_triggered: Vec<String>,
}

/// Stress scenarios to inject (Right View: test impermanence)
#[derive(Debug, Clone)]
pub enum StressScenario {
    /// Sudden price gap (flash crash / spike)
    PriceGap { direction: f64, magnitude_pct: f64 },
    /// Spread widens dramatically
    SpreadBlowout { multiplier: f64 },
    /// Liquidity vacuum (no fills)
    LiquidityVacuum { duration_bars: u32 },
    /// Volatility regime change
    VolatilitySpike { multiplier: f64 },
    /// Funding rate extreme
    FundingExtreme { rate: f64 },
    /// Data delay / staleness
    DataDelay { delay_ms: u64 },
    /// Exchange downtime
    Outage { duration_bars: u32 },
    /// Cascading liquidations
    LiquidationCascade { intensity: f64 },
}

impl StressScenario {
    /// Standard stress test suite
    pub fn standard_suite() -> Vec<Self> {
        vec![
            // Flash crash: -10% instant
            StressScenario::PriceGap { direction: -1.0, magnitude_pct: 0.10 },
            // Flash spike: +10% instant
            StressScenario::PriceGap { direction: 1.0, magnitude_pct: 0.10 },
            // Spread blows out 10x
            StressScenario::SpreadBlowout { multiplier: 10.0 },
            // No fills for 5 bars
            StressScenario::LiquidityVacuum { duration_bars: 5 },
            // Volatility triples
            StressScenario::VolatilitySpike { multiplier: 3.0 },
            // Extreme negative funding
            StressScenario::FundingExtreme { rate: -0.01 },
            // Extreme positive funding
            StressScenario::FundingExtreme { rate: 0.01 },
            // Data delayed 30 seconds
            StressScenario::DataDelay { delay_ms: 30_000 },
            // Exchange down for 10 bars
            StressScenario::Outage { duration_bars: 10 },
            // Liquidation cascade
            StressScenario::LiquidationCascade { intensity: 5.0 },
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            StressScenario::PriceGap { direction, .. } =>
                if *direction < 0.0 { "flash_crash" } else { "flash_spike" },
            StressScenario::SpreadBlowout { .. } => "spread_blowout",
            StressScenario::LiquidityVacuum { .. } => "liquidity_vacuum",
            StressScenario::VolatilitySpike { .. } => "volatility_spike",
            StressScenario::FundingExtreme { rate, .. } =>
                if *rate < 0.0 { "funding_negative_extreme" } else { "funding_positive_extreme" },
            StressScenario::DataDelay { .. } => "data_delay",
            StressScenario::Outage { .. } => "exchange_outage",
            StressScenario::LiquidationCascade { .. } => "liquidation_cascade",
        }
    }

    pub fn description(&self) -> String {
        match self {
            StressScenario::PriceGap { direction, magnitude_pct } =>
                format!("Price {} {:.0}% instantly",
                    if *direction < 0.0 { "drops" } else { "spikes" },
                    magnitude_pct * 100.0),
            StressScenario::SpreadBlowout { multiplier } =>
                format!("Spread widens {:.0}x", multiplier),
            StressScenario::LiquidityVacuum { duration_bars } =>
                format!("No fills for {} bars", duration_bars),
            StressScenario::VolatilitySpike { multiplier } =>
                format!("Volatility increases {:.0}x", multiplier),
            StressScenario::FundingExtreme { rate } =>
                format!("Funding rate hits {:.2}%", rate * 100.0),
            StressScenario::DataDelay { delay_ms } =>
                format!("Data delayed {}ms", delay_ms),
            StressScenario::Outage { duration_bars } =>
                format!("Exchange down for {} bars", duration_bars),
            StressScenario::LiquidationCascade { intensity } =>
                format!("Liquidation cascade intensity {:.1}x", intensity),
        }
    }
}

/// Walk-forward validation configuration (Right View: no peeking)
#[derive(Debug, Clone)]
pub struct WalkForwardConfig {
    /// Training window size in bars
    pub train_bars: usize,
    /// Test window size in bars
    pub test_bars: usize,
    /// Step size between windows
    pub step_bars: usize,
    /// Minimum number of folds
    pub min_folds: usize,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            train_bars: 100,
            test_bars: 20,
            step_bars: 20,
            min_folds: 5,
        }
    }
}

/// Result of walk-forward validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    /// Results from each fold
    pub fold_results: Vec<FoldResult>,
    /// Average out-of-sample return
    pub avg_oos_return: f64,
    /// Std dev of out-of-sample returns
    pub std_oos_return: f64,
    /// Ratio of OOS return to in-sample return
    pub oos_to_is_ratio: f64,
    /// Number of folds where OOS < 0
    pub negative_folds: u32,
}

impl WalkForwardResult {
    /// Detect overfitting: OOS much worse than IS suggests delusion
    pub fn overfitting_score(&self) -> f64 {
        // Perfect = 1.0 (OOS equals IS)
        // Overfit = 0.0 (OOS much worse than IS)
        self.oos_to_is_ratio.min(1.0).max(0.0)
    }

    /// Is this result trustworthy?
    pub fn is_trustworthy(&self) -> bool {
        // At least 60% OOS/IS ratio
        // Less than half the folds negative
        // Positive average OOS return
        let oos_ratio_ok = self.oos_to_is_ratio > 0.6;
        let folds_ok = (self.negative_folds as f64) / (self.fold_results.len().max(1) as f64) < 0.5;
        let return_ok = self.avg_oos_return > 0.0;
        oos_ratio_ok && folds_ok && return_ok
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldResult {
    pub fold_index: usize,
    pub train_start: u64,
    pub train_end: u64,
    pub test_start: u64,
    pub test_end: u64,
    pub in_sample_return: f64,
    pub out_of_sample_return: f64,
}

/// Lookahead leak detector (Right Action: no deception)
pub struct LookaheadDetector {
    /// Timestamp of current decision point
    decision_ts: u64,
    /// Timestamps of data actually used
    data_access_log: Vec<(u64, String)>,
    /// Violations found
    violations: Vec<LookaheadViolation>,
}

#[derive(Debug, Clone)]
pub struct LookaheadViolation {
    pub decision_ts: u64,
    pub data_ts: u64,
    pub data_source: String,
    pub description: String,
}

impl LookaheadDetector {
    pub fn new() -> Self {
        Self {
            decision_ts: 0,
            data_access_log: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// Set the current decision timestamp
    pub fn set_decision_time(&mut self, ts: u64) {
        self.decision_ts = ts;
        self.data_access_log.clear();
    }

    /// Log a data access and check for lookahead
    pub fn log_data_access(&mut self, data_ts: u64, source: &str) {
        self.data_access_log.push((data_ts, source.to_string()));

        // Check for violation
        if data_ts > self.decision_ts {
            self.violations.push(LookaheadViolation {
                decision_ts: self.decision_ts,
                data_ts,
                data_source: source.to_string(),
                description: format!(
                    "Decision at {} used data from {} (future by {}ms)",
                    self.decision_ts, data_ts, data_ts - self.decision_ts
                ),
            });
        }
    }

    pub fn violations(&self) -> &[LookaheadViolation] {
        &self.violations
    }

    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }
}

impl Default for LookaheadDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute percentile of a sorted slice
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookahead_detector() {
        let mut detector = LookaheadDetector::new();

        // Set decision at t=100
        detector.set_decision_time(100);

        // Access data from t=90 (ok)
        detector.log_data_access(90, "price");
        assert!(!detector.has_violations());

        // Access data from t=110 (violation!)
        detector.log_data_access(110, "future_price");
        assert!(detector.has_violations());
        assert_eq!(detector.violations().len(), 1);
    }

    #[test]
    fn test_reality_score_penalizes_drawdown() {
        let config = ExperimentConfig {
            id: "test".to_string(),
            seed: 42,
            params: HashMap::new(),
            start_ts: 0,
            end_ts: 1000,
            code_version: None,
            run_ts: 0,
        };

        // Good system: low drawdown
        let good = BacktestResults {
            config: config.clone(),
            total_return_pct: 0.10,
            sharpe_ratio: 1.5,
            total_trades: 50,
            max_drawdown_pct: 0.05,
            max_drawdown_duration: 10,
            worst_trade_pct: 0.01,
            max_consecutive_losses: 2,
            loss_95th_percentile: 0.005,
            loss_99th_percentile: 0.01,
            high_vol_return: 0.08,
            low_vol_return: 0.12,
            trending_return: 0.10,
            ranging_return: 0.10,
            stress_results: vec![
                StressTestResult {
                    name: "test".to_string(),
                    description: "test".to_string(),
                    survived: true,
                    stress_drawdown_pct: 0.05,
                    guards_triggered: vec![],
                },
            ],
            lookahead_violations: 0,
            data_gaps: 0,
            final_state_hash: 0,
        };

        // Bad system: high drawdown
        let bad = BacktestResults {
            max_drawdown_pct: 0.40,
            loss_99th_percentile: 0.10,
            stress_results: vec![
                StressTestResult {
                    name: "test".to_string(),
                    description: "test".to_string(),
                    survived: false,
                    stress_drawdown_pct: 0.30,
                    guards_triggered: vec![],
                },
            ],
            ..good.clone()
        };

        assert!(good.reality_score() > bad.reality_score());
    }

    #[test]
    fn test_alignment_assessment_detects_corruption() {
        let config = ExperimentConfig {
            id: "test".to_string(),
            seed: 42,
            params: HashMap::new(),
            start_ts: 0,
            end_ts: 1000,
            code_version: None,
            run_ts: 0,
        };

        let corrupted = BacktestResults {
            config,
            total_return_pct: 1.0, // Amazing returns!
            sharpe_ratio: 5.0,
            total_trades: 100,
            max_drawdown_pct: 0.01,
            max_drawdown_duration: 1,
            worst_trade_pct: 0.001,
            max_consecutive_losses: 0,
            loss_95th_percentile: 0.001,
            loss_99th_percentile: 0.002,
            high_vol_return: 1.0,
            low_vol_return: 1.0,
            trending_return: 1.0,
            ranging_return: 1.0,
            stress_results: vec![],
            lookahead_violations: 1, // But corrupted!
            data_gaps: 0,
            final_state_hash: 0,
        };

        assert!(matches!(
            corrupted.alignment_assessment(),
            AlignmentAssessment::Corrupted { .. }
        ));
    }
}
