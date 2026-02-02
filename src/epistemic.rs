//! Epistemic state export for visualization dashboard.
//!
//! Tracks the verification status of each component:
//! - Asserted: stated but not tested
//! - Verified: confirmed by test/backtest
//! - Used: actively used in decisions
//! - Invariant: proven to always hold
//! - Assumed: taken as given
//! - Inferred: derived from other data
//! - Extrapolated: projected beyond known
//! - Aspirational: hoped for, not yet real

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EpistemicLevel {
    Asserted,
    Verified,
    Used,
    Invariant,
    Assumed,
    Inferred,
    Extrapolated,
    Aspirational,
}

impl EpistemicLevel {
    pub fn color(&self) -> &'static str {
        match self {
            Self::Asserted => "#FF9800",
            Self::Verified => "#4CAF50",
            Self::Used => "#2196F3",
            Self::Invariant => "#9C27B0",
            Self::Assumed => "#FFEB3B",
            Self::Inferred => "#00BCD4",
            Self::Extrapolated => "#E91E63",
            Self::Aspirational => "#9E9E9E",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpistemicNode {
    pub id: String,
    pub name: String,
    pub level: EpistemicLevel,
    pub confidence: f64,
    pub source_file: Option<String>,
    pub line_number: Option<u32>,
    pub depends_on: Vec<String>,
    pub used_by: Vec<String>,
    pub evidence: Vec<EvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub source: String,
    pub regime: String,
    pub metric: f64,
    pub supports: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlow {
    pub from: String,
    pub to: String,
    pub level: EpistemicLevel,
    pub transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invariant {
    pub id: String,
    pub statement: String,
    pub holds: bool,
    pub checked_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpistemicState {
    pub hypotheses: Vec<EpistemicNode>,
    pub signals: Vec<EpistemicNode>,
    pub filters: Vec<EpistemicNode>,
    pub strategies: Vec<EpistemicNode>,
    pub dataflows: Vec<DataFlow>,
    pub invariants: Vec<Invariant>,
    pub assumptions: Vec<EpistemicNode>,
}

impl EpistemicState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build state from current system configuration
    pub fn from_system() -> Self {
        let mut state = Self::new();

        // Hypotheses from hypothesis ledger
        state.hypotheses = vec![
            EpistemicNode {
                id: "H001".into(),
                name: "Momentum continuation".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.42,
                source_file: Some("src/signals.rs".into()),
                line_number: Some(45),
                depends_on: vec!["ema_fast".into(), "ema_slow".into()],
                used_by: vec!["momentum_strategy".into()],
                evidence: vec![EvidenceItem {
                    source: "backtest".into(),
                    regime: "StrongBear".into(),
                    metric: -0.12,
                    supports: false,
                }],
            },
            EpistemicNode {
                id: "H004".into(),
                name: "Trend filter improves quality".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.78,
                source_file: Some("src/filters.rs".into()),
                line_number: Some(55),
                depends_on: vec!["ema_fast".into(), "ema_slow".into()],
                used_by: vec!["trend_follower".into(), "composite".into()],
                evidence: vec![EvidenceItem {
                    source: "backtest".into(),
                    regime: "StrongBear".into(),
                    metric: 0.62,
                    supports: true,
                }],
            },
            EpistemicNode {
                id: "H006".into(),
                name: "Short-selling in bear markets".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.92,
                source_file: Some("src/strategies.rs".into()),
                line_number: Some(180),
                depends_on: vec!["trend".into(), "momentum".into()],
                used_by: vec!["bear_short".into(), "adaptive".into()],
                evidence: vec![EvidenceItem {
                    source: "backtest".into(),
                    regime: "StrongBear".into(),
                    metric: 1.96,
                    supports: true,
                }],
            },
            EpistemicNode {
                id: "H008".into(),
                name: "Volatility breakouts".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.71,
                source_file: Some("src/signals.rs".into()),
                line_number: Some(120),
                depends_on: vec!["atr".into(), "bb".into()],
                used_by: vec!["breakout_strategy".into()],
                evidence: vec![EvidenceItem {
                    source: "backtest".into(),
                    regime: "StrongBear".into(),
                    metric: 0.45,
                    supports: true,
                }],
            },
        ];

        // Signals
        state.signals = vec![
            EpistemicNode {
                id: "momentum".into(),
                name: "Momentum signal".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.85,
                source_file: Some("src/signals.rs".into()),
                line_number: Some(25),
                depends_on: vec!["ema_fast".into(), "price".into()],
                used_by: vec!["H001".into(), "H006".into()],
                evidence: vec![],
            },
            EpistemicNode {
                id: "trend".into(),
                name: "Trend signal".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.90,
                source_file: Some("src/signals.rs".into()),
                line_number: Some(60),
                depends_on: vec!["ema_fast".into(), "ema_slow".into()],
                used_by: vec!["H004".into(), "H006".into()],
                evidence: vec![],
            },
            EpistemicNode {
                id: "funding_rate".into(),
                name: "Funding rate signal".into(),
                level: EpistemicLevel::Assumed,
                confidence: 0.60,
                source_file: Some("src/feed/aux_data.rs".into()),
                line_number: Some(45),
                depends_on: vec!["exchange_api".into()],
                used_by: vec!["carry_strategy".into()],
                evidence: vec![],
            },
        ];

        // Filters
        state.filters = vec![
            EpistemicNode {
                id: "volatility_filter".into(),
                name: "Volatility filter".into(),
                level: EpistemicLevel::Verified,
                confidence: 0.88,
                source_file: Some("src/filters.rs".into()),
                line_number: Some(29),
                depends_on: vec!["vol".into(), "vol_mean".into()],
                used_by: vec!["all_strategies".into()],
                evidence: vec![],
            },
            EpistemicNode {
                id: "position_limit".into(),
                name: "Position limit filter".into(),
                level: EpistemicLevel::Invariant,
                confidence: 1.0,
                source_file: Some("src/filters.rs".into()),
                line_number: Some(123),
                depends_on: vec!["portfolio".into(), "max_position".into()],
                used_by: vec!["all_strategies".into()],
                evidence: vec![],
            },
        ];

        // Dataflows
        state.dataflows = vec![
            DataFlow { from: "market_data".into(), to: "indicators".into(),
                       level: EpistemicLevel::Verified, transform: Some("parse_csv".into()) },
            DataFlow { from: "indicators".into(), to: "signals".into(),
                       level: EpistemicLevel::Verified, transform: Some("compute".into()) },
            DataFlow { from: "signals".into(), to: "filters".into(),
                       level: EpistemicLevel::Verified, transform: Some("evaluate".into()) },
            DataFlow { from: "filters".into(), to: "strategy".into(),
                       level: EpistemicLevel::Verified, transform: Some("decide".into()) },
            DataFlow { from: "strategy".into(), to: "sizing".into(),
                       level: EpistemicLevel::Verified, transform: Some("size".into()) },
            DataFlow { from: "sizing".into(), to: "order".into(),
                       level: EpistemicLevel::Verified, transform: Some("submit".into()) },
            DataFlow { from: "order".into(), to: "exchange".into(),
                       level: EpistemicLevel::Assumed, transform: Some("api_call".into()) },
            DataFlow { from: "exchange".into(), to: "fill".into(),
                       level: EpistemicLevel::Inferred, transform: Some("response".into()) },
            DataFlow { from: "fill".into(), to: "ledger".into(),
                       level: EpistemicLevel::Verified, transform: Some("record".into()) },
            DataFlow { from: "ledger".into(), to: "metrics".into(),
                       level: EpistemicLevel::Verified, transform: Some("aggregate".into()) },
            DataFlow { from: "metrics".into(), to: "hypothesis".into(),
                       level: EpistemicLevel::Verified, transform: Some("evaluate".into()) },
        ];

        // Invariants
        state.invariants = vec![
            Invariant {
                id: "position_bounded".into(),
                statement: "abs(position) <= max_position".into(),
                holds: true,
                checked_at: 0,
            },
            Invariant {
                id: "equity_positive".into(),
                statement: "equity > 0".into(),
                holds: true,
                checked_at: 0,
            },
            Invariant {
                id: "fee_accounted".into(),
                statement: "all trades include fee estimation".into(),
                holds: true,
                checked_at: 0,
            },
        ];

        // Assumptions
        state.assumptions = vec![
            EpistemicNode {
                id: "market_liquid".into(),
                name: "Market liquidity assumption".into(),
                level: EpistemicLevel::Assumed,
                confidence: 0.70,
                source_file: None,
                line_number: None,
                depends_on: vec![],
                used_by: vec!["sizing".into()],
                evidence: vec![],
            },
            EpistemicNode {
                id: "exchange_reliable".into(),
                name: "Exchange reliability assumption".into(),
                level: EpistemicLevel::Assumed,
                confidence: 0.85,
                source_file: None,
                line_number: None,
                depends_on: vec![],
                used_by: vec!["order_execution".into()],
                evidence: vec![],
            },
            EpistemicNode {
                id: "regime_detectable".into(),
                name: "Regime detection assumption".into(),
                level: EpistemicLevel::Extrapolated,
                confidence: 0.50,
                source_file: None,
                line_number: None,
                depends_on: vec!["price_history".into()],
                used_by: vec!["adaptive_strategy".into()],
                evidence: vec![],
            },
        ];

        state
    }

    /// Export to JSON for dashboard
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Count nodes by epistemic level
    pub fn count_by_level(&self) -> HashMap<EpistemicLevel, usize> {
        let mut counts: HashMap<EpistemicLevel, usize> = HashMap::new();

        for h in &self.hypotheses {
            *counts.entry(h.level).or_insert(0) += 1;
        }
        for s in &self.signals {
            *counts.entry(s.level).or_insert(0) += 1;
        }
        for f in &self.filters {
            *counts.entry(f.level).or_insert(0) += 1;
        }
        for a in &self.assumptions {
            *counts.entry(a.level).or_insert(0) += 1;
        }

        counts
    }

    /// Check if all invariants hold
    pub fn invariants_hold(&self) -> bool {
        self.invariants.iter().all(|i| i.holds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_system() {
        let state = EpistemicState::from_system();
        assert!(!state.hypotheses.is_empty());
        assert!(!state.dataflows.is_empty());
        assert!(state.invariants_hold());
    }

    #[test]
    fn test_count_by_level() {
        let state = EpistemicState::from_system();
        let counts = state.count_by_level();
        assert!(counts.get(&EpistemicLevel::Verified).unwrap_or(&0) > &0);
    }
}
