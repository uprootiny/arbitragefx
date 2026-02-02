//! Hypothesis and Evidence Ledger for systematic strategy research.
//!
//! This module provides a framework for:
//! - Formulating testable trading hypotheses
//! - Recording evidence from backtests
//! - Tracking hypothesis status (confirmed/refuted/inconclusive)
//! - Driving strategy refinement based on evidence

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Hypothesis status based on accumulated evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HypothesisStatus {
    Proposed,       // Initial state
    Testing,        // Currently being tested
    Supported,      // Evidence supports hypothesis
    Refuted,        // Evidence contradicts hypothesis
    Inconclusive,   // Mixed or insufficient evidence
    Superseded,     // Replaced by refined hypothesis
}

/// Market regime classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    StrongBull,     // >20% gain
    ModerateBull,   // 5-20% gain
    Ranging,        // -5% to 5%
    ModerateBear,   // -5% to -20%
    StrongBear,     // >20% loss
    HighVolatility, // Regime-agnostic high vol
    LowVolatility,  // Regime-agnostic low vol
}

impl MarketRegime {
    pub fn from_price_change(pct: f64) -> Self {
        if pct > 20.0 { Self::StrongBull }
        else if pct > 5.0 { Self::ModerateBull }
        else if pct > -5.0 { Self::Ranging }
        else if pct > -20.0 { Self::ModerateBear }
        else { Self::StrongBear }
    }

    pub fn from_volatility_ratio(vol_ratio: f64) -> Self {
        if vol_ratio > 2.0 { Self::HighVolatility }
        else if vol_ratio < 0.5 { Self::LowVolatility }
        else { Self::Ranging }
    }
}

/// A testable hypothesis about market behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub statement: String,
    pub rationale: String,
    pub testable_prediction: String,
    pub success_criteria: SuccessCriteria,
    pub status: HypothesisStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub parent_id: Option<String>,  // If refined from another hypothesis
    pub tags: Vec<String>,
}

/// Criteria for evaluating hypothesis success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriteria {
    pub min_trades: u32,
    pub min_win_rate: Option<f64>,
    pub min_sharpe: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub min_profit_factor: Option<f64>,
    pub applies_to_regime: Option<MarketRegime>,
}

impl Default for SuccessCriteria {
    fn default() -> Self {
        Self {
            min_trades: 10,
            min_win_rate: Some(0.5),
            min_sharpe: Some(0.5),
            max_drawdown: Some(-0.15),
            min_profit_factor: Some(1.2),
            applies_to_regime: None,
        }
    }
}

/// Evidence from a single backtest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub hypothesis_id: String,
    pub timestamp: u64,
    pub data_source: String,
    pub regime: MarketRegime,
    pub metrics: BacktestMetrics,
    pub supports_hypothesis: Option<bool>,
    pub notes: String,
}

/// Metrics from a backtest run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub pnl: f64,
    pub trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f64,
    pub sharpe: f64,
    pub max_drawdown: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub expectancy: f64,
    pub bars_tested: u64,
    pub execution_time_ms: u64,
}

impl BacktestMetrics {
    /// Calculate derived metrics
    pub fn finalize(&mut self) {
        if self.trades > 0 {
            self.win_rate = self.wins as f64 / self.trades as f64;
        }
        if self.avg_loss.abs() > 0.0 {
            self.profit_factor = self.avg_win / self.avg_loss.abs();
        }
        // Expectancy = (Win% * AvgWin) - (Loss% * AvgLoss)
        self.expectancy = (self.win_rate * self.avg_win)
            - ((1.0 - self.win_rate) * self.avg_loss.abs());
    }

    /// Check if metrics meet success criteria
    pub fn meets_criteria(&self, criteria: &SuccessCriteria) -> bool {
        if self.trades < criteria.min_trades {
            return false;
        }
        if let Some(min_wr) = criteria.min_win_rate {
            if self.win_rate < min_wr {
                return false;
            }
        }
        if let Some(min_sharpe) = criteria.min_sharpe {
            if self.sharpe < min_sharpe {
                return false;
            }
        }
        if let Some(max_dd) = criteria.max_drawdown {
            if self.max_drawdown < max_dd {
                return false;
            }
        }
        if let Some(min_pf) = criteria.min_profit_factor {
            if self.profit_factor < min_pf {
                return false;
            }
        }
        true
    }
}

/// The hypothesis ledger - tracks all hypotheses and evidence
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HypothesisLedger {
    pub hypotheses: HashMap<String, Hypothesis>,
    pub evidence: Vec<Evidence>,
    pub regime_performance: HashMap<MarketRegime, RegimeStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegimeStats {
    pub tests_run: u32,
    pub avg_sharpe: f64,
    pub avg_win_rate: f64,
    pub best_strategy: Option<String>,
    pub best_sharpe: f64,
}

impl HypothesisLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new hypothesis
    pub fn add_hypothesis(&mut self, h: Hypothesis) {
        self.hypotheses.insert(h.id.clone(), h);
    }

    /// Record evidence from a backtest
    pub fn record_evidence(&mut self, mut e: Evidence) {
        // Check if evidence supports or refutes hypothesis
        if let Some(h) = self.hypotheses.get(&e.hypothesis_id) {
            e.supports_hypothesis = Some(e.metrics.meets_criteria(&h.success_criteria));
        }
        self.evidence.push(e);
    }

    /// Update hypothesis status based on evidence
    pub fn update_hypothesis_status(&mut self, hypothesis_id: &str) {
        let evidence: Vec<_> = self.evidence
            .iter()
            .filter(|e| e.hypothesis_id == hypothesis_id)
            .collect();

        if evidence.is_empty() {
            return;
        }

        let supports: usize = evidence.iter()
            .filter(|e| e.supports_hypothesis == Some(true))
            .count();
        let refutes: usize = evidence.iter()
            .filter(|e| e.supports_hypothesis == Some(false))
            .count();

        let new_status = if evidence.len() < 3 {
            HypothesisStatus::Testing
        } else if supports as f64 / evidence.len() as f64 > 0.7 {
            HypothesisStatus::Supported
        } else if refutes as f64 / evidence.len() as f64 > 0.7 {
            HypothesisStatus::Refuted
        } else {
            HypothesisStatus::Inconclusive
        };

        if let Some(h) = self.hypotheses.get_mut(hypothesis_id) {
            h.status = new_status;
        }
    }

    /// Get hypotheses by status
    pub fn by_status(&self, status: HypothesisStatus) -> Vec<&Hypothesis> {
        self.hypotheses.values()
            .filter(|h| h.status == status)
            .collect()
    }

    /// Suggest next experiments based on gaps in evidence
    pub fn suggest_experiments(&self) -> Vec<SuggestedExperiment> {
        let mut suggestions = Vec::new();

        // Find hypotheses that need more testing
        for h in self.by_status(HypothesisStatus::Testing) {
            let evidence_count = self.evidence.iter()
                .filter(|e| e.hypothesis_id == h.id)
                .count();
            if evidence_count < 5 {
                suggestions.push(SuggestedExperiment {
                    hypothesis_id: h.id.clone(),
                    reason: format!("Only {} evidence points, need at least 5", evidence_count),
                    suggested_regime: None,
                });
            }
        }

        // Find regime gaps
        for h in self.hypotheses.values() {
            if h.status == HypothesisStatus::Supported || h.status == HypothesisStatus::Testing {
                let tested_regimes: Vec<_> = self.evidence.iter()
                    .filter(|e| e.hypothesis_id == h.id)
                    .map(|e| e.regime)
                    .collect();

                for regime in [MarketRegime::StrongBull, MarketRegime::StrongBear,
                               MarketRegime::Ranging, MarketRegime::HighVolatility] {
                    if !tested_regimes.contains(&regime) {
                        suggestions.push(SuggestedExperiment {
                            hypothesis_id: h.id.clone(),
                            reason: format!("Not tested in {:?} regime", regime),
                            suggested_regime: Some(regime),
                        });
                    }
                }
            }
        }

        suggestions
    }

    /// Generate summary report
    pub fn summary(&self) -> LedgerSummary {
        let mut by_status: HashMap<HypothesisStatus, u32> = HashMap::new();
        for h in self.hypotheses.values() {
            *by_status.entry(h.status).or_insert(0) += 1;
        }

        let mut best_by_regime: HashMap<MarketRegime, (String, f64)> = HashMap::new();
        for e in &self.evidence {
            let entry = best_by_regime.entry(e.regime).or_insert((String::new(), f64::MIN));
            if e.metrics.sharpe > entry.1 {
                *entry = (e.hypothesis_id.clone(), e.metrics.sharpe);
            }
        }

        LedgerSummary {
            total_hypotheses: self.hypotheses.len(),
            by_status,
            total_evidence: self.evidence.len(),
            best_by_regime,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SuggestedExperiment {
    pub hypothesis_id: String,
    pub reason: String,
    pub suggested_regime: Option<MarketRegime>,
}

#[derive(Debug, Clone)]
pub struct LedgerSummary {
    pub total_hypotheses: usize,
    pub by_status: HashMap<HypothesisStatus, u32>,
    pub total_evidence: usize,
    pub best_by_regime: HashMap<MarketRegime, (String, f64)>,
}

// =============================================================================
// Standard Trading Hypotheses
// =============================================================================

/// Generate standard trading hypotheses to test
pub fn standard_hypotheses() -> Vec<Hypothesis> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    vec![
        Hypothesis {
            id: "H001_momentum".into(),
            statement: "Positive momentum predicts continued price increases".into(),
            rationale: "Trend persistence - assets in motion tend to stay in motion".into(),
            testable_prediction: "Long positions on z_momentum > 1.0 should be profitable".into(),
            success_criteria: SuccessCriteria {
                min_trades: 20,
                min_win_rate: Some(0.45),
                min_sharpe: Some(0.3),
                max_drawdown: Some(-0.25),
                min_profit_factor: Some(1.1),
                applies_to_regime: Some(MarketRegime::ModerateBull),
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["momentum".into(), "trend".into()],
        },
        Hypothesis {
            id: "H002_mean_reversion".into(),
            statement: "Extreme price deviations mean-revert".into(),
            rationale: "Overextension leads to retracement as buyers/sellers exhaust".into(),
            testable_prediction: "Counter-trend entries at z_stretch > 2.0 should profit".into(),
            success_criteria: SuccessCriteria {
                min_trades: 15,
                min_win_rate: Some(0.55),
                min_sharpe: Some(0.4),
                max_drawdown: Some(-0.15),
                min_profit_factor: Some(1.2),
                applies_to_regime: Some(MarketRegime::Ranging),
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["mean_reversion".into(), "contrarian".into()],
        },
        Hypothesis {
            id: "H003_volatility_scaling".into(),
            statement: "Position sizing inverse to volatility improves risk-adjusted returns".into(),
            rationale: "Equal risk per trade regardless of market conditions".into(),
            testable_prediction: "Vol-scaled positions have better Sharpe than fixed sizing".into(),
            success_criteria: SuccessCriteria {
                min_trades: 30,
                min_win_rate: None,
                min_sharpe: Some(0.5),
                max_drawdown: Some(-0.20),
                min_profit_factor: None,
                applies_to_regime: None,
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["sizing".into(), "volatility".into()],
        },
        Hypothesis {
            id: "H004_trend_filter".into(),
            statement: "Filtering trades by EMA trend direction reduces losses".into(),
            rationale: "Trading with the trend has higher success probability".into(),
            testable_prediction: "Trend-aligned trades have higher win rate than unfiltered".into(),
            success_criteria: SuccessCriteria {
                min_trades: 10,
                min_win_rate: Some(0.40),  // Trend-filtered may have fewer but better trades
                min_sharpe: Some(0.4),
                max_drawdown: Some(-0.25),
                min_profit_factor: Some(1.0),
                applies_to_regime: None,
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["filter".into(), "trend".into()],
        },
        Hypothesis {
            id: "H005_rsi_extremes".into(),
            statement: "RSI extremes (<30, >70) predict reversals".into(),
            rationale: "Overbought/oversold conditions indicate exhaustion".into(),
            testable_prediction: "Counter-RSI trades at extremes should be profitable".into(),
            success_criteria: SuccessCriteria {
                min_trades: 15,
                min_win_rate: Some(0.50),
                min_sharpe: Some(0.3),
                max_drawdown: Some(-0.20),
                min_profit_factor: Some(1.15),
                applies_to_regime: None,
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["rsi".into(), "mean_reversion".into()],
        },
        Hypothesis {
            id: "H006_bear_short".into(),
            statement: "Short-selling in bear markets outperforms going long".into(),
            rationale: "Align with dominant market direction".into(),
            testable_prediction: "Short-biased strategy beats long-only in >-15% markets".into(),
            success_criteria: SuccessCriteria {
                min_trades: 10,
                min_win_rate: Some(0.35),  // Lower win rate ok if profits are large
                min_sharpe: Some(0.5),     // Higher Sharpe requirement
                max_drawdown: Some(-0.35), // Allow more drawdown in bear
                min_profit_factor: Some(1.0),
                applies_to_regime: Some(MarketRegime::StrongBear),
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["short".into(), "bear_market".into()],
        },
        Hypothesis {
            id: "H007_confluence".into(),
            statement: "Multiple confirming indicators improve trade quality".into(),
            rationale: "Independent signals aligning reduces false positives".into(),
            testable_prediction: "3+ indicator confluence has higher win rate than single".into(),
            success_criteria: SuccessCriteria {
                min_trades: 10,
                min_win_rate: Some(0.55),
                min_sharpe: Some(0.5),
                max_drawdown: Some(-0.15),
                min_profit_factor: Some(1.3),
                applies_to_regime: None,
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["confluence".into(), "multi_indicator".into()],
        },
        Hypothesis {
            id: "H008_vol_breakout".into(),
            statement: "Volatility expansions from low-vol periods predict direction".into(),
            rationale: "Consolidation breakouts carry momentum".into(),
            testable_prediction: "Trades on vol_ratio > 1.5 from vol_ratio < 0.7 are profitable".into(),
            success_criteria: SuccessCriteria {
                min_trades: 8,
                min_win_rate: Some(0.38),  // Breakouts have lower win rate but higher R:R
                min_sharpe: Some(0.35),
                max_drawdown: Some(-0.25),
                min_profit_factor: Some(1.1),
                applies_to_regime: None,
            },
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: None,
            tags: vec!["breakout".into(), "volatility".into()],
        },
    ]
}

// =============================================================================
// Refinement and Recombination
// =============================================================================

/// A refinement of an existing hypothesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refinement {
    pub original_id: String,
    pub refined_id: String,
    pub change_description: String,
    pub rationale: String,
    pub created_at: u64,
}

/// A combination of multiple hypotheses into a composite strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Combination {
    pub id: String,
    pub name: String,
    pub component_ids: Vec<String>,
    pub combination_logic: String,
    pub regime_selector: Option<String>,
    pub status: HypothesisStatus,
    pub evidence: Vec<String>,  // Evidence IDs
}

/// Development action derived from evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevelopmentAction {
    /// Refine hypothesis with specific change
    Refine {
        hypothesis_id: String,
        suggested_change: String,
        rationale: String,
    },
    /// Combine hypotheses
    Combine {
        hypothesis_ids: Vec<String>,
        combination_logic: String,
    },
    /// Test in new regime
    TestRegime {
        hypothesis_id: String,
        regime: MarketRegime,
    },
    /// Abandon hypothesis
    Abandon {
        hypothesis_id: String,
        reason: String,
    },
    /// Promote to production
    Promote {
        hypothesis_id: String,
    },
}

impl HypothesisLedger {
    /// Derive development actions from current evidence
    pub fn derive_actions(&self) -> Vec<DevelopmentAction> {
        let mut actions = Vec::new();

        for (id, h) in &self.hypotheses {
            let evidence: Vec<_> = self.evidence.iter()
                .filter(|e| e.hypothesis_id == *id)
                .collect();

            if evidence.is_empty() {
                continue;
            }

            // Analyze evidence patterns
            let supports: Vec<_> = evidence.iter()
                .filter(|e| e.supports_hypothesis == Some(true))
                .collect();
            let refutes: Vec<_> = evidence.iter()
                .filter(|e| e.supports_hypothesis == Some(false))
                .collect();

            // High Sharpe but refuted on win rate -> refine criteria
            for e in &refutes {
                if e.metrics.sharpe > 0.5 && e.metrics.pnl > 0.0 {
                    actions.push(DevelopmentAction::Refine {
                        hypothesis_id: id.clone(),
                        suggested_change: "Relax win rate requirement".into(),
                        rationale: format!(
                            "Sharpe {:.2} with positive PnL suggests strategy works despite low win rate",
                            e.metrics.sharpe
                        ),
                    });
                }
            }

            // Strong in one regime, untested in others
            let tested_regimes: Vec<_> = evidence.iter().map(|e| e.regime).collect();
            if !supports.is_empty() {
                for regime in [MarketRegime::StrongBull, MarketRegime::StrongBear,
                               MarketRegime::Ranging, MarketRegime::HighVolatility] {
                    if !tested_regimes.contains(&regime) {
                        actions.push(DevelopmentAction::TestRegime {
                            hypothesis_id: id.clone(),
                            regime,
                        });
                    }
                }
            }

            // Consistently refuted with negative PnL -> abandon
            if refutes.len() >= 3 && supports.is_empty() {
                let all_negative = refutes.iter().all(|e| e.metrics.pnl < 0.0);
                if all_negative {
                    actions.push(DevelopmentAction::Abandon {
                        hypothesis_id: id.clone(),
                        reason: "Consistently negative PnL across tests".into(),
                    });
                }
            }

            // Strong support across regimes -> promote
            if supports.len() >= 3 {
                let regimes: std::collections::HashSet<_> = supports.iter()
                    .map(|e| e.regime)
                    .collect();
                if regimes.len() >= 2 {
                    actions.push(DevelopmentAction::Promote {
                        hypothesis_id: id.clone(),
                    });
                }
            }
        }

        // Look for combination opportunities
        let supported: Vec<_> = self.hypotheses.values()
            .filter(|h| {
                let evidence: Vec<_> = self.evidence.iter()
                    .filter(|e| e.hypothesis_id == h.id && e.supports_hypothesis == Some(true))
                    .collect();
                !evidence.is_empty()
            })
            .collect();

        // Find complementary hypotheses (different regimes)
        if supported.len() >= 2 {
            let mut regime_specialists: HashMap<MarketRegime, Vec<String>> = HashMap::new();
            for h in &supported {
                if let Some(regime) = h.success_criteria.applies_to_regime {
                    regime_specialists.entry(regime).or_default().push(h.id.clone());
                }
            }

            // If we have specialists for both bull and bear, suggest combination
            if regime_specialists.contains_key(&MarketRegime::StrongBull)
                && regime_specialists.contains_key(&MarketRegime::StrongBear) {
                let bull_id = &regime_specialists[&MarketRegime::StrongBull][0];
                let bear_id = &regime_specialists[&MarketRegime::StrongBear][0];
                actions.push(DevelopmentAction::Combine {
                    hypothesis_ids: vec![bull_id.clone(), bear_id.clone()],
                    combination_logic: "Use regime detector to switch between strategies".into(),
                });
            }
        }

        actions
    }

    /// Create a refined hypothesis from an existing one
    pub fn refine(&mut self, original_id: &str, changes: HypothesisChanges) -> Option<String> {
        let original = self.hypotheses.get(original_id)?.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let refined_id = format!("{}_v{}", original_id, now % 10000);

        let refined = Hypothesis {
            id: refined_id.clone(),
            statement: changes.statement.unwrap_or(original.statement),
            rationale: changes.rationale.unwrap_or(original.rationale),
            testable_prediction: changes.testable_prediction.unwrap_or(original.testable_prediction),
            success_criteria: changes.success_criteria.unwrap_or(original.success_criteria),
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
            parent_id: Some(original_id.to_string()),
            tags: original.tags,
        };

        // Mark original as superseded
        if let Some(h) = self.hypotheses.get_mut(original_id) {
            h.status = HypothesisStatus::Superseded;
            h.updated_at = now;
        }

        self.hypotheses.insert(refined_id.clone(), refined);
        Some(refined_id)
    }

    /// Get refinement chain for a hypothesis
    pub fn refinement_chain(&self, hypothesis_id: &str) -> Vec<&Hypothesis> {
        let mut chain = Vec::new();
        let mut current_id = Some(hypothesis_id.to_string());

        while let Some(id) = current_id {
            if let Some(h) = self.hypotheses.get(&id) {
                chain.push(h);
                current_id = h.parent_id.clone();
            } else {
                break;
            }
        }

        chain.reverse();
        chain
    }

    /// Get performance summary across regimes
    pub fn regime_performance(&self, hypothesis_id: &str) -> HashMap<MarketRegime, f64> {
        let mut perf: HashMap<MarketRegime, Vec<f64>> = HashMap::new();

        for e in &self.evidence {
            if e.hypothesis_id == hypothesis_id {
                perf.entry(e.regime).or_default().push(e.metrics.sharpe);
            }
        }

        perf.into_iter()
            .map(|(regime, sharpes)| {
                let avg = sharpes.iter().sum::<f64>() / sharpes.len() as f64;
                (regime, avg)
            })
            .collect()
    }

    /// Find best hypothesis for a given regime
    pub fn best_for_regime(&self, regime: MarketRegime) -> Option<(&Hypothesis, f64)> {
        let mut best: Option<(&Hypothesis, f64)> = None;

        for h in self.hypotheses.values() {
            let evidence: Vec<_> = self.evidence.iter()
                .filter(|e| e.hypothesis_id == h.id && e.regime == regime)
                .collect();

            if evidence.is_empty() {
                continue;
            }

            let avg_sharpe = evidence.iter()
                .map(|e| e.metrics.sharpe)
                .sum::<f64>() / evidence.len() as f64;

            if best.is_none() || avg_sharpe > best.unwrap().1 {
                best = Some((h, avg_sharpe));
            }
        }

        best
    }
}

/// Changes to apply when refining a hypothesis
#[derive(Debug, Clone, Default)]
pub struct HypothesisChanges {
    pub statement: Option<String>,
    pub rationale: Option<String>,
    pub testable_prediction: Option<String>,
    pub success_criteria: Option<SuccessCriteria>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regime_classification() {
        assert_eq!(MarketRegime::from_price_change(25.0), MarketRegime::StrongBull);
        assert_eq!(MarketRegime::from_price_change(-30.0), MarketRegime::StrongBear);
        assert_eq!(MarketRegime::from_price_change(0.0), MarketRegime::Ranging);
    }

    #[test]
    fn test_metrics_criteria() {
        let metrics = BacktestMetrics {
            trades: 20,
            win_rate: 0.55,
            sharpe: 0.8,
            max_drawdown: -0.10,
            profit_factor: 1.5,
            ..Default::default()
        };

        let criteria = SuccessCriteria::default();
        assert!(metrics.meets_criteria(&criteria));

        let strict = SuccessCriteria {
            min_sharpe: Some(1.0),
            ..Default::default()
        };
        assert!(!metrics.meets_criteria(&strict));
    }

    #[test]
    fn test_ledger_evidence() {
        let mut ledger = HypothesisLedger::new();

        let h = Hypothesis {
            id: "test".into(),
            statement: "Test hypothesis".into(),
            rationale: "Testing".into(),
            testable_prediction: "Test".into(),
            success_criteria: SuccessCriteria::default(),
            status: HypothesisStatus::Testing,
            created_at: 0,
            updated_at: 0,
            parent_id: None,
            tags: vec![],
        };
        ledger.add_hypothesis(h);

        let e = Evidence {
            id: "e1".into(),
            hypothesis_id: "test".into(),
            timestamp: 0,
            data_source: "test".into(),
            regime: MarketRegime::Ranging,
            metrics: BacktestMetrics {
                trades: 15,
                win_rate: 0.6,
                sharpe: 0.7,
                max_drawdown: -0.08,
                profit_factor: 1.4,
                ..Default::default()
            },
            supports_hypothesis: None,
            notes: "Test evidence".into(),
        };
        ledger.record_evidence(e);

        assert_eq!(ledger.evidence.len(), 1);
        assert_eq!(ledger.evidence[0].supports_hypothesis, Some(true));
    }
}
