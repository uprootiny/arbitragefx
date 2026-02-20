//! Narrative Detector - Distinguishing signal from collective delusion
//!
//! In high-feedback speculative environments, price action can be driven by:
//! - Genuine information discovery
//! - Reflexive narrative loops (delusion)
//!
//! This module detects when the environment is dominated by narrative
//! rather than grounded conditions, triggering defensive postures.
//!
//! ## Design Principle
//!
//! > "The danger is not the market. It's being pulled into narrative
//! >  while believing you're acting on data."

use serde::{Serialize, Deserialize};

/// Narrative regime classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarrativeRegime {
    /// Price action appears grounded in observable flows
    Grounded,
    /// Mixed signals - some narrative influence
    Uncertain,
    /// Strong signs of narrative-driven action
    NarrativeDriven,
    /// Extreme reflexivity - bubble/panic dynamics
    Reflexive,
}

impl NarrativeRegime {
    /// Should we reduce exposure in this regime?
    pub fn should_reduce_exposure(&self) -> bool {
        matches!(self, NarrativeRegime::NarrativeDriven | NarrativeRegime::Reflexive)
    }

    /// Maximum position size multiplier for this regime
    pub fn position_multiplier(&self) -> f64 {
        match self {
            NarrativeRegime::Grounded => 1.0,
            NarrativeRegime::Uncertain => 0.7,
            NarrativeRegime::NarrativeDriven => 0.3,
            NarrativeRegime::Reflexive => 0.0, // No new positions
        }
    }
}

/// Indicators of narrative-driven markets
#[derive(Debug, Clone, Default)]
pub struct NarrativeIndicators {
    // === Funding Rate Extremes ===
    /// Current funding rate
    pub funding_rate: f64,
    /// Historical average funding
    pub funding_avg: f64,
    /// Funding z-score
    pub funding_zscore: f64,

    // === Liquidation Dynamics ===
    /// Recent liquidation volume (normalized)
    pub liquidation_score: f64,
    /// Liquidation imbalance (long vs short)
    pub liquidation_imbalance: f64,

    // === Price-Volume Divergence ===
    /// Price change magnitude
    pub price_change_pct: f64,
    /// Volume relative to average
    pub volume_ratio: f64,
    /// Price-volume correlation breakdown
    pub pv_divergence: f64,

    // === Volatility Regime ===
    /// Current volatility vs historical
    pub volatility_ratio: f64,
    /// Volatility clustering (autocorrelation)
    pub vol_clustering: f64,

    // === Social/Sentiment Proxies ===
    /// Open interest change rate
    pub oi_change_rate: f64,
    /// New account inflows (if available)
    pub retail_flow_proxy: f64,
}

impl NarrativeIndicators {
    /// Compute narrative score (0.0 = grounded, 1.0 = highly narrative)
    pub fn narrative_score(&self) -> f64 {
        let mut score = 0.0;
        let mut weight_sum = 0.0;

        // Funding extremes suggest crowded trades
        // High funding = everyone long = narrative "number go up"
        let funding_signal = (self.funding_zscore.abs() / 3.0).min(1.0);
        score += funding_signal * 0.25;
        weight_sum += 0.25;

        // High liquidations suggest forced selling, not information
        let liq_signal = (self.liquidation_score / 2.0).min(1.0);
        score += liq_signal * 0.20;
        weight_sum += 0.20;

        // Price moves without volume suggest narrative, not flow
        let divergence_signal = self.pv_divergence.abs().min(1.0);
        score += divergence_signal * 0.15;
        weight_sum += 0.15;

        // Extreme volatility suggests panic/euphoria
        let vol_signal = ((self.volatility_ratio - 1.0) / 2.0).max(0.0).min(1.0);
        score += vol_signal * 0.20;
        weight_sum += 0.20;

        // Rapid OI changes suggest speculative inflows
        let oi_signal = (self.oi_change_rate.abs() / 0.1).min(1.0);
        score += oi_signal * 0.20;
        weight_sum += 0.20;

        if weight_sum > 0.0 {
            score / weight_sum
        } else {
            0.0
        }
    }

    /// Classify current regime
    pub fn regime(&self) -> NarrativeRegime {
        let score = self.narrative_score();

        if score < 0.25 {
            NarrativeRegime::Grounded
        } else if score < 0.50 {
            NarrativeRegime::Uncertain
        } else if score < 0.75 {
            NarrativeRegime::NarrativeDriven
        } else {
            NarrativeRegime::Reflexive
        }
    }

    /// Generate defensive actions based on regime
    pub fn defensive_actions(&self) -> Vec<DefensiveAction> {
        let mut actions = Vec::new();
        let regime = self.regime();

        match regime {
            NarrativeRegime::Grounded => {
                // Normal operation
            }
            NarrativeRegime::Uncertain => {
                actions.push(DefensiveAction::ReducePositionSize { multiplier: 0.7 });
                actions.push(DefensiveAction::TightenStops { multiplier: 0.8 });
            }
            NarrativeRegime::NarrativeDriven => {
                actions.push(DefensiveAction::ReducePositionSize { multiplier: 0.3 });
                actions.push(DefensiveAction::TightenStops { multiplier: 0.6 });
                actions.push(DefensiveAction::IncreaseCooldown { multiplier: 2.0 });
                actions.push(DefensiveAction::LogWarning {
                    msg: "Narrative-driven regime detected. Reducing exposure.".to_string(),
                });
            }
            NarrativeRegime::Reflexive => {
                actions.push(DefensiveAction::HaltNewPositions);
                actions.push(DefensiveAction::ReduceExisting { target_pct: 0.5 });
                actions.push(DefensiveAction::LogWarning {
                    msg: "Reflexive regime detected. Halting new positions.".to_string(),
                });
            }
        }

        // Specific indicator triggers
        if self.funding_zscore.abs() > 2.5 {
            actions.push(DefensiveAction::LogWarning {
                msg: format!("Extreme funding: z={:.2}. Crowded trade warning.", self.funding_zscore),
            });
        }

        if self.liquidation_score > 3.0 {
            actions.push(DefensiveAction::LogWarning {
                msg: format!("Liquidation cascade: score={:.2}. Forced flow dominant.", self.liquidation_score),
            });
        }

        actions
    }
}

/// Defensive actions triggered by narrative detection
#[derive(Debug, Clone)]
pub enum DefensiveAction {
    /// Reduce position size by multiplier
    ReducePositionSize { multiplier: f64 },
    /// Tighten stop losses
    TightenStops { multiplier: f64 },
    /// Increase cooldown between trades
    IncreaseCooldown { multiplier: f64 },
    /// Halt new position entries
    HaltNewPositions,
    /// Reduce existing positions to target
    ReduceExisting { target_pct: f64 },
    /// Log warning for audit trail
    LogWarning { msg: String },
}

/// Self-deception detector
///
/// Monitors operator behavior patterns that indicate drift into narrative.
///
/// ## Warning Signs
/// - Overriding risk limits after wins
/// - Increasing position sizes during volatility
/// - Shortening cooldowns after losses
/// - Changing parameters to match recent performance
#[derive(Debug, Clone, Default)]
pub struct SelfDeceptionDetector {
    /// Recent parameter changes
    pub param_changes: Vec<ParamChange>,
    /// Risk limit overrides
    pub risk_overrides: u32,
    /// Position size trend (should be flat or down in volatility)
    pub size_trend_in_vol: f64,
    /// Cooldown shortening events
    pub cooldown_shortenings: u32,
}

#[derive(Debug, Clone)]
pub struct ParamChange {
    pub timestamp: u64,
    pub param_name: String,
    pub old_value: f64,
    pub new_value: f64,
    pub after_win: bool,
}

impl SelfDeceptionDetector {
    /// Compute self-deception risk score
    pub fn deception_score(&self) -> f64 {
        let mut score = 0.0;

        // Risk overrides are concerning
        score += (self.risk_overrides as f64 * 0.2).min(0.4);

        // Size increasing in volatility is a red flag
        if self.size_trend_in_vol > 0.0 {
            score += (self.size_trend_in_vol * 0.3).min(0.3);
        }

        // Cooldown shortening suggests impatience
        score += (self.cooldown_shortenings as f64 * 0.15).min(0.3);

        // Parameter changes after wins suggest narrative fitting
        let post_win_changes = self.param_changes.iter()
            .filter(|c| c.after_win)
            .count();
        score += (post_win_changes as f64 * 0.1).min(0.3);

        score.min(1.0)
    }

    /// Is operator showing signs of narrative capture?
    pub fn is_captured(&self) -> bool {
        self.deception_score() > 0.5
    }

    /// Generate warnings
    pub fn warnings(&self) -> Vec<String> {
        let mut w = Vec::new();

        if self.risk_overrides > 0 {
            w.push(format!(
                "Risk limits overridden {} times. This erodes the system's integrity.",
                self.risk_overrides
            ));
        }

        if self.size_trend_in_vol > 0.1 {
            w.push(
                "Position sizes increasing during volatility. This is backwards - \
                 high vol should mean smaller sizes.".to_string()
            );
        }

        if self.cooldown_shortenings > 0 {
            w.push(format!(
                "Cooldowns shortened {} times. Impatience is a symptom of narrative capture.",
                self.cooldown_shortenings
            ));
        }

        let post_win_changes: Vec<_> = self.param_changes.iter()
            .filter(|c| c.after_win)
            .collect();
        if !post_win_changes.is_empty() {
            w.push(format!(
                "Parameters changed after wins: {:?}. This is curve-fitting to luck.",
                post_win_changes.iter().map(|c| &c.param_name).collect::<Vec<_>>()
            ));
        }

        w
    }
}

/// Bubble resistance configuration
///
/// These parameters define how the system behaves as narrative intensity increases.
#[derive(Debug, Clone)]
pub struct BubbleResistanceConfig {
    /// At what narrative score do we start reducing exposure?
    pub reduction_threshold: f64,
    /// At what narrative score do we halt new positions?
    pub halt_threshold: f64,
    /// Minimum position multiplier even in extreme regimes
    pub floor_multiplier: f64,
    /// How quickly to reduce positions when entering narrative regime
    pub reduction_rate: f64,
    /// Funding z-score that triggers crowded trade warning
    pub funding_warning_zscore: f64,
    /// Liquidation score that triggers cascade warning
    pub liquidation_warning_threshold: f64,
}

impl Default for BubbleResistanceConfig {
    fn default() -> Self {
        Self {
            reduction_threshold: 0.4,
            halt_threshold: 0.75,
            floor_multiplier: 0.1,
            reduction_rate: 0.5,
            funding_warning_zscore: 2.0,
            liquidation_warning_threshold: 2.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grounded_regime() {
        let indicators = NarrativeIndicators {
            funding_zscore: 0.5,
            liquidation_score: 0.3,
            volatility_ratio: 1.1,
            ..Default::default()
        };

        assert_eq!(indicators.regime(), NarrativeRegime::Grounded);
        assert!(!indicators.regime().should_reduce_exposure());
    }

    #[test]
    fn test_reflexive_regime() {
        let indicators = NarrativeIndicators {
            funding_zscore: 4.0,  // Extreme funding
            liquidation_score: 5.0,  // Heavy liquidations
            volatility_ratio: 3.0,  // Triple normal vol
            oi_change_rate: 0.2,  // Rapid OI change
            ..Default::default()
        };

        assert_eq!(indicators.regime(), NarrativeRegime::Reflexive);
        assert!(indicators.regime().should_reduce_exposure());
        assert_eq!(indicators.regime().position_multiplier(), 0.0);
    }

    #[test]
    fn test_defensive_actions_narrative_driven() {
        // Create indicators that trigger NarrativeDriven (not Reflexive)
        let indicators = NarrativeIndicators {
            funding_zscore: 2.0,    // Moderate
            liquidation_score: 1.5, // Moderate
            volatility_ratio: 1.8,  // Moderate
            oi_change_rate: 0.08,   // Moderate
            pv_divergence: 0.3,
            ..Default::default()
        };

        let score = indicators.narrative_score();
        let regime = indicators.regime();

        // Should be in NarrativeDriven range (0.50 - 0.75)
        assert!(score >= 0.50 && score < 0.75,
            "Score {} should be in NarrativeDriven range", score);
        assert_eq!(regime, NarrativeRegime::NarrativeDriven);

        let actions = indicators.defensive_actions();
        assert!(!actions.is_empty());

        // Should include position reduction for NarrativeDriven
        let has_reduction = actions.iter().any(|a| matches!(a, DefensiveAction::ReducePositionSize { .. }));
        assert!(has_reduction, "NarrativeDriven should reduce position size");
    }

    #[test]
    fn test_defensive_actions_reflexive() {
        // Create indicators that trigger Reflexive regime
        let indicators = NarrativeIndicators {
            funding_zscore: 4.0,    // Extreme
            liquidation_score: 5.0, // Extreme
            volatility_ratio: 3.0,  // Triple normal
            oi_change_rate: 0.2,    // Rapid change
            pv_divergence: 0.8,
            ..Default::default()
        };

        let score = indicators.narrative_score();
        let regime = indicators.regime();

        // Should be Reflexive (>= 0.75)
        assert!(score >= 0.75, "Score {} should be >= 0.75 for Reflexive", score);
        assert_eq!(regime, NarrativeRegime::Reflexive);

        let actions = indicators.defensive_actions();

        // Reflexive should halt new positions
        let has_halt = actions.iter().any(|a| matches!(a, DefensiveAction::HaltNewPositions));
        assert!(has_halt, "Reflexive regime should halt new positions");
    }

    #[test]
    fn test_self_deception_detection() {
        let mut detector = SelfDeceptionDetector::default();

        // Simulate concerning behavior
        detector.risk_overrides = 2;
        detector.size_trend_in_vol = 0.3;
        detector.cooldown_shortenings = 1;
        detector.param_changes.push(ParamChange {
            timestamp: 1000,
            param_name: "entry_threshold".to_string(),
            old_value: 0.3,
            new_value: 0.2,
            after_win: true,
        });

        assert!(detector.deception_score() > 0.5);
        assert!(detector.is_captured());
        assert!(!detector.warnings().is_empty());
    }
}
