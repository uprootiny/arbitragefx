//! Policy trait: the contract between agent and engine.
//!
//! The agent outputs **Intent**, not raw orders.
//! Intent flows through the risk gate before becoming execution.
//!
//! > "The agent proposes; the gate disposes."

use serde::{Deserialize, Serialize};

/// What the agent sees each decision step.
#[derive(Debug, Clone)]
pub struct AgentInput {
    /// Current market snapshot (strictly as-of time)
    pub market: MarketSnapshot,
    /// Current portfolio state
    pub portfolio: PortfolioSnapshot,
    /// Risk/regime state
    pub risk: RiskSnapshot,
    /// Wall clock timestamp
    pub ts: u64,
}

/// Strictly as-of market data.
#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub symbol: String,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    /// Computed features with provenance
    pub z_momentum: f64,
    pub z_mean_deviation: f64,
    pub rsi: f64,
    pub volatility: f64,
    pub funding_rate: f64,
    /// Data age in ms
    pub staleness_ms: u64,
    /// Feature window size (for provenance)
    pub window_size: usize,
    /// Missing data flags
    pub has_gaps: bool,
}

/// Portfolio state visible to agent.
#[derive(Debug, Clone, Default)]
pub struct PortfolioSnapshot {
    /// Current position quantity (signed)
    pub position: f64,
    /// Average entry price
    pub entry_price: f64,
    /// Unrealized PnL
    pub unrealized_pnl: f64,
    /// Realized PnL (session)
    pub realized_pnl: f64,
    /// Current equity
    pub equity: f64,
    /// Effective leverage
    pub leverage: f64,
}

/// Risk state visible to agent.
#[derive(Debug, Clone)]
pub struct RiskSnapshot {
    /// Current regime label
    pub regime: RegimeLabel,
    /// Regime confidence (0.0 = unknown, 1.0 = certain)
    pub regime_confidence: f64,
    /// Drift severity
    pub drift_severity: DriftLabel,
    /// Position multiplier from regime + drift
    pub position_multiplier: f64,
    /// Is trading halted?
    pub halted: bool,
    /// Halt reason if halted
    pub halt_reason: Option<String>,
    /// Active cooldowns
    pub cooldowns: Vec<CooldownInfo>,
    /// Consecutive errors
    pub consecutive_errors: u32,
}

impl Default for RiskSnapshot {
    fn default() -> Self {
        Self {
            regime: RegimeLabel::Unknown,
            regime_confidence: 0.0,
            drift_severity: DriftLabel::None,
            position_multiplier: 1.0,
            halted: false,
            halt_reason: None,
            cooldowns: vec![],
            consecutive_errors: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimeLabel {
    Unknown,
    Grounded,
    Uncertain,
    NarrativeDriven,
    Reflexive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftLabel {
    None,
    Low,
    Moderate,
    Severe,
    Critical,
}

#[derive(Debug, Clone)]
pub struct CooldownInfo {
    pub name: String,
    pub remaining_ms: u64,
}

/// What the agent outputs: Intent, not raw orders.
///
/// The risk gate converts Intent → Command (or rejects).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// The intended action
    pub intent: Intent,
    /// Required: why this intent? (Right Speech constraint)
    pub rationale: String,
    /// Confidence level (0.0 = no confidence, 1.0 = certain)
    pub confidence: f64,
}

/// Intent types - these go through the risk gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Intent {
    /// Target a specific exposure level
    TargetExposure {
        symbol: String,
        /// Target delta as fraction of equity (-1.0 to 1.0)
        target_delta: f64,
    },
    /// Go flat (close position)
    Flat { symbol: String },
    /// Reduce risk without specific target
    ReduceRisk {
        symbol: String,
        /// How much to reduce (0.0 to 1.0)
        reduction_factor: f64,
    },
    /// Hold current position
    Hold,
    /// Request halt (agent recognizes uncertainty)
    RequestHalt { reason: String },
}

/// Risk gate decision.
#[derive(Debug, Clone)]
pub enum GateDecision {
    /// Intent approved, execute this command
    Approved {
        intent: Intent,
        adjusted_size: f64,
        reason: String,
    },
    /// Intent modified due to constraints
    Modified {
        original: Intent,
        adjusted: Intent,
        reason: String,
    },
    /// Intent rejected
    Rejected {
        intent: Intent,
        reason: String,
    },
}

/// The Policy trait: what an agent must implement.
pub trait Policy: Send + Sync {
    /// Generate intent from current state.
    ///
    /// The agent sees a sanitized snapshot and returns an intent
    /// with a mandatory rationale.
    fn decide(&self, input: &AgentInput) -> AgentOutput;

    /// Policy name for logging.
    fn name(&self) -> &str;
}

/// Risk gate that sits between policy and execution.
pub struct RiskGate {
    /// Maximum position as fraction of equity
    pub max_position_fraction: f64,
    /// Maximum trades per day
    pub max_trades_per_day: u32,
    /// Minimum ms between trades
    pub min_trade_interval_ms: u64,
    /// Halt on consecutive errors threshold
    pub halt_on_errors: u32,
    /// Minimum confidence to act
    pub min_confidence: f64,
}

impl Default for RiskGate {
    fn default() -> Self {
        Self {
            max_position_fraction: 0.1,
            max_trades_per_day: 20,
            min_trade_interval_ms: 60_000,
            halt_on_errors: 3,
            min_confidence: 0.3,
        }
    }
}

impl RiskGate {
    /// Evaluate an intent through the risk gate.
    pub fn evaluate(
        &self,
        output: &AgentOutput,
        input: &AgentInput,
        trades_today: u32,
        last_trade_ts: u64,
    ) -> GateDecision {
        // Check halt
        if input.risk.halted {
            return GateDecision::Rejected {
                intent: output.intent.clone(),
                reason: format!(
                    "System halted: {}",
                    input.risk.halt_reason.as_deref().unwrap_or("unknown")
                ),
            };
        }

        // Check confidence
        if output.confidence < self.min_confidence {
            return GateDecision::Rejected {
                intent: output.intent.clone(),
                reason: format!(
                    "Confidence {:.2} below minimum {:.2}",
                    output.confidence, self.min_confidence
                ),
            };
        }

        // Check trade rate
        if trades_today >= self.max_trades_per_day {
            return GateDecision::Rejected {
                intent: output.intent.clone(),
                reason: format!("Trade limit reached: {}/{}", trades_today, self.max_trades_per_day),
            };
        }

        // Check trade interval
        let ms_since_trade = input.ts.saturating_sub(last_trade_ts);
        if ms_since_trade < self.min_trade_interval_ms {
            return GateDecision::Rejected {
                intent: output.intent.clone(),
                reason: format!(
                    "Cooldown: {}ms remaining",
                    self.min_trade_interval_ms - ms_since_trade
                ),
            };
        }

        // Check staleness
        if input.market.staleness_ms > 30_000 {
            return GateDecision::Rejected {
                intent: output.intent.clone(),
                reason: format!("Data stale: {}ms", input.market.staleness_ms),
            };
        }

        // Apply position multiplier from regime/drift
        let multiplier = input.risk.position_multiplier;

        match &output.intent {
            Intent::TargetExposure { symbol, target_delta } => {
                let adjusted_delta = target_delta * multiplier;
                let clamped = adjusted_delta.clamp(
                    -self.max_position_fraction,
                    self.max_position_fraction,
                );

                if (clamped - target_delta).abs() > 0.001 {
                    GateDecision::Modified {
                        original: output.intent.clone(),
                        adjusted: Intent::TargetExposure {
                            symbol: symbol.clone(),
                            target_delta: clamped,
                        },
                        reason: format!(
                            "Position adjusted: {:.2} → {:.2} (multiplier={:.2}, max={:.2})",
                            target_delta, clamped, multiplier, self.max_position_fraction
                        ),
                    }
                } else {
                    GateDecision::Approved {
                        intent: output.intent.clone(),
                        adjusted_size: clamped,
                        reason: "Within limits".to_string(),
                    }
                }
            }

            Intent::Flat { .. } | Intent::ReduceRisk { .. } => {
                // Risk-reducing actions always approved
                GateDecision::Approved {
                    intent: output.intent.clone(),
                    adjusted_size: 0.0,
                    reason: "Risk reduction approved".to_string(),
                }
            }

            Intent::Hold => GateDecision::Approved {
                intent: output.intent.clone(),
                adjusted_size: 0.0,
                reason: "Hold approved".to_string(),
            },

            Intent::RequestHalt { reason } => GateDecision::Approved {
                intent: output.intent.clone(),
                adjusted_size: 0.0,
                reason: format!("Agent requested halt: {}", reason),
            },
        }
    }
}

/// Minimal mean-reversion policy (the default agent).
pub struct MeanReversionPolicy {
    /// Entry threshold for z-score
    pub entry_z: f64,
    /// Exit threshold for z-score
    pub exit_z: f64,
    /// Base position size as fraction of equity
    pub base_size: f64,
}

impl Default for MeanReversionPolicy {
    fn default() -> Self {
        Self {
            entry_z: 2.0,
            exit_z: 0.5,
            base_size: 0.05,
        }
    }
}

impl Policy for MeanReversionPolicy {
    fn name(&self) -> &str {
        "mean_reversion_v1"
    }

    fn decide(&self, input: &AgentInput) -> AgentOutput {
        let z = input.market.z_mean_deviation;
        let pos = input.portfolio.position;

        // Unknown regime = hold with explanation
        if matches!(input.risk.regime, RegimeLabel::Unknown) {
            return AgentOutput {
                intent: Intent::Hold,
                rationale: "Regime unknown; holding until clarity".to_string(),
                confidence: 0.1,
            };
        }

        // High drift = reduce or hold
        if matches!(input.risk.drift_severity, DriftLabel::Severe | DriftLabel::Critical) {
            if pos.abs() > 0.001 {
                return AgentOutput {
                    intent: Intent::Flat {
                        symbol: input.market.symbol.clone(),
                    },
                    rationale: format!(
                        "Drift {:?} detected; closing position for safety",
                        input.risk.drift_severity
                    ),
                    confidence: 0.8,
                };
            } else {
                return AgentOutput {
                    intent: Intent::Hold,
                    rationale: format!(
                        "Drift {:?} detected; no new positions",
                        input.risk.drift_severity
                    ),
                    confidence: 0.8,
                };
            }
        }

        // Mean reversion logic
        if pos.abs() < 0.001 {
            // No position - look for entry
            if z > self.entry_z {
                // Price extended high - short opportunity
                AgentOutput {
                    intent: Intent::TargetExposure {
                        symbol: input.market.symbol.clone(),
                        target_delta: -self.base_size,
                    },
                    rationale: format!(
                        "Mean reversion SHORT: z={:.2} > threshold {:.2}; expecting pullback",
                        z, self.entry_z
                    ),
                    confidence: (z / 4.0).min(0.9),
                }
            } else if z < -self.entry_z {
                // Price extended low - long opportunity
                AgentOutput {
                    intent: Intent::TargetExposure {
                        symbol: input.market.symbol.clone(),
                        target_delta: self.base_size,
                    },
                    rationale: format!(
                        "Mean reversion LONG: z={:.2} < threshold -{:.2}; expecting bounce",
                        z, self.entry_z
                    ),
                    confidence: (z.abs() / 4.0).min(0.9),
                }
            } else {
                AgentOutput {
                    intent: Intent::Hold,
                    rationale: format!(
                        "No signal: z={:.2} within ±{:.2} threshold",
                        z, self.entry_z
                    ),
                    confidence: 0.5,
                }
            }
        } else {
            // Have position - look for exit
            let _in_profit = (pos > 0.0 && z > -self.exit_z) || (pos < 0.0 && z < self.exit_z);

            if z.abs() < self.exit_z {
                // Mean reverted - take profit
                AgentOutput {
                    intent: Intent::Flat {
                        symbol: input.market.symbol.clone(),
                    },
                    rationale: format!(
                        "Mean reversion complete: z={:.2} within ±{:.2}; closing",
                        z, self.exit_z
                    ),
                    confidence: 0.7,
                }
            } else if (pos > 0.0 && z > self.entry_z) || (pos < 0.0 && z < -self.entry_z) {
                // Wrong side - stop out
                AgentOutput {
                    intent: Intent::Flat {
                        symbol: input.market.symbol.clone(),
                    },
                    rationale: format!(
                        "Stop: position {} but z={:.2} moved against; cutting loss",
                        if pos > 0.0 { "LONG" } else { "SHORT" },
                        z
                    ),
                    confidence: 0.9,
                }
            } else {
                AgentOutput {
                    intent: Intent::Hold,
                    rationale: format!(
                        "Holding {} position: z={:.2}, waiting for mean reversion",
                        if pos > 0.0 { "LONG" } else { "SHORT" },
                        z
                    ),
                    confidence: 0.5,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(z: f64, position: f64, regime: RegimeLabel, drift: DriftLabel) -> AgentInput {
        AgentInput {
            market: MarketSnapshot {
                symbol: "BTCUSDT".to_string(),
                price: 50000.0,
                bid: 49999.0,
                ask: 50001.0,
                z_momentum: 0.0,
                z_mean_deviation: z,
                rsi: 50.0,
                volatility: 0.02,
                funding_rate: 0.0001,
                staleness_ms: 100,
                window_size: 20,
                has_gaps: false,
            },
            portfolio: PortfolioSnapshot {
                position,
                entry_price: 50000.0,
                unrealized_pnl: 0.0,
                realized_pnl: 0.0,
                equity: 10000.0,
                leverage: 1.0,
            },
            risk: RiskSnapshot {
                regime,
                regime_confidence: 0.8,
                drift_severity: drift,
                position_multiplier: drift.position_multiplier(),
                halted: false,
                halt_reason: None,
                cooldowns: vec![],
                consecutive_errors: 0,
            },
            ts: 1000000,
        }
    }

    impl DriftLabel {
        fn position_multiplier(&self) -> f64 {
            match self {
                DriftLabel::None => 1.0,
                DriftLabel::Low => 0.9,
                DriftLabel::Moderate => 0.5,
                DriftLabel::Severe => 0.0,
                DriftLabel::Critical => 0.0,
            }
        }
    }

    #[test]
    fn test_policy_unknown_regime_holds() {
        let policy = MeanReversionPolicy::default();
        let input = make_input(3.0, 0.0, RegimeLabel::Unknown, DriftLabel::None);
        let output = policy.decide(&input);

        assert!(matches!(output.intent, Intent::Hold));
        assert!(output.rationale.contains("unknown"));
    }

    #[test]
    fn test_policy_severe_drift_flattens() {
        let policy = MeanReversionPolicy::default();
        let input = make_input(0.0, 1.0, RegimeLabel::Grounded, DriftLabel::Severe);
        let output = policy.decide(&input);

        assert!(matches!(output.intent, Intent::Flat { .. }));
        assert!(output.rationale.contains("Drift"));
    }

    #[test]
    fn test_policy_entry_long() {
        let policy = MeanReversionPolicy::default();
        let input = make_input(-2.5, 0.0, RegimeLabel::Grounded, DriftLabel::None);
        let output = policy.decide(&input);

        if let Intent::TargetExposure { target_delta, .. } = output.intent {
            assert!(target_delta > 0.0, "Should go long on negative z");
        } else {
            panic!("Expected TargetExposure");
        }
    }

    #[test]
    fn test_policy_entry_short() {
        let policy = MeanReversionPolicy::default();
        let input = make_input(2.5, 0.0, RegimeLabel::Grounded, DriftLabel::None);
        let output = policy.decide(&input);

        if let Intent::TargetExposure { target_delta, .. } = output.intent {
            assert!(target_delta < 0.0, "Should go short on positive z");
        } else {
            panic!("Expected TargetExposure");
        }
    }

    #[test]
    fn test_gate_rejects_low_confidence() {
        let gate = RiskGate::default();
        let input = make_input(0.0, 0.0, RegimeLabel::Grounded, DriftLabel::None);
        let output = AgentOutput {
            intent: Intent::TargetExposure {
                symbol: "BTCUSDT".to_string(),
                target_delta: 0.05,
            },
            rationale: "test".to_string(),
            confidence: 0.1, // Below threshold
        };

        let decision = gate.evaluate(&output, &input, 0, 0);
        assert!(matches!(decision, GateDecision::Rejected { .. }));
    }

    #[test]
    fn test_gate_modifies_excessive_size() {
        let gate = RiskGate {
            max_position_fraction: 0.05,
            ..Default::default()
        };
        let input = make_input(0.0, 0.0, RegimeLabel::Grounded, DriftLabel::None);
        let output = AgentOutput {
            intent: Intent::TargetExposure {
                symbol: "BTCUSDT".to_string(),
                target_delta: 0.2, // Exceeds max
            },
            rationale: "test".to_string(),
            confidence: 0.8,
        };

        let decision = gate.evaluate(&output, &input, 0, 0);
        assert!(matches!(decision, GateDecision::Modified { .. }));
    }

    #[test]
    fn test_gate_approves_risk_reduction() {
        let gate = RiskGate::default();
        let mut input = make_input(0.0, 0.0, RegimeLabel::Grounded, DriftLabel::None);
        input.risk.halted = false;

        let output = AgentOutput {
            intent: Intent::Flat {
                symbol: "BTCUSDT".to_string(),
            },
            rationale: "closing for safety".to_string(),
            confidence: 0.9,
        };

        let decision = gate.evaluate(&output, &input, 0, 0);
        assert!(matches!(decision, GateDecision::Approved { .. }));
    }
}
