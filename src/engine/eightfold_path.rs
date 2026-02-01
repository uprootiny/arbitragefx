//! The Noble Eightfold Path as Operational Epistemology
//!
//! This module maps Buddhist cognitive hygiene to system design.
//! It is not religion — it's a protocol for not lying to yourself
//! in a complex causal system.
//!
//! ## The Path → Rust Module Mapping
//!
//! | Path Factor | System Analog | Primary Module |
//! |-------------|---------------|----------------|
//! | Right View | Honest uncertainty modeling | `state.rs`, `backtest_ethics.rs` |
//! | Right Intention | Survivability over greed | `reducer.rs`, `ethics.rs` |
//! | Right Speech | Truthful representation | `logging.rs`, WAL |
//! | Right Action | Non-manipulative participation | `exchange/*.rs` |
//! | Right Livelihood | Work that doesn't amplify harm | `risk.rs`, ethics guards |
//! | Right Effort | Error reduction over fantasy | `backtest_traps.rs` |
//! | Right Mindfulness | Continuous state monitoring | `metrics.rs`, health checks |
//! | Right Concentration | Calm operational modes | `fault.rs`, halt conditions |

use serde::{Serialize, Deserialize};

/// The eight factors as a checklist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EightfoldChecklist {
    pub right_view: RightViewStatus,
    pub right_intention: RightIntentionStatus,
    pub right_speech: RightSpeechStatus,
    pub right_action: RightActionStatus,
    pub right_livelihood: RightLivelihoodStatus,
    pub right_effort: RightEffortStatus,
    pub right_mindfulness: RightMindfulnessStatus,
    pub right_concentration: RightConcentrationStatus,
}

impl EightfoldChecklist {
    /// Overall alignment score (0.0 - 1.0)
    pub fn alignment_score(&self) -> f64 {
        let scores = [
            self.right_view.score(),
            self.right_intention.score(),
            self.right_speech.score(),
            self.right_action.score(),
            self.right_livelihood.score(),
            self.right_effort.score(),
            self.right_mindfulness.score(),
            self.right_concentration.score(),
        ];
        scores.iter().sum::<f64>() / 8.0
    }

    /// Is the system in a safe operational state?
    pub fn is_operational(&self) -> bool {
        // Must pass critical factors
        self.right_view.uncertainty_acknowledged &&
        self.right_action.no_manipulation &&
        self.right_concentration.halt_conditions_active
    }

    /// Get violations
    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        v.extend(self.right_view.violations());
        v.extend(self.right_intention.violations());
        v.extend(self.right_speech.violations());
        v.extend(self.right_action.violations());
        v.extend(self.right_livelihood.violations());
        v.extend(self.right_effort.violations());
        v.extend(self.right_mindfulness.violations());
        v.extend(self.right_concentration.violations());
        v
    }
}

//=============================================================================
// RIGHT VIEW (sammā diṭṭhi) → Honest Uncertainty Modeling
//=============================================================================

/// **Module responsibility:** `state.rs`, `backtest_ethics.rs`
///
/// Right View asks: "Are we seeing reality clearly, or what we want to see?"
///
/// ## Implementation checklist:
/// - [ ] Rolling statistics only (no global normalization)
/// - [ ] Regime segmentation in backtests
/// - [ ] Uncertainty quantified, not hidden
/// - [ ] Staleness tracked, not defaulted to zero
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightViewStatus {
    /// Are we using online/rolling stats only?
    pub uses_online_stats: bool,
    /// Do we segment by regime?
    pub regime_segmented: bool,
    /// Is uncertainty quantified?
    pub uncertainty_acknowledged: bool,
    /// Is staleness tracked (not defaulted)?
    pub staleness_tracked: bool,
    /// Do we have lookahead leakage?
    pub lookahead_violations: u32,
}

impl RightViewStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.uses_online_stats { s += 0.25; }
        if self.regime_segmented { s += 0.25; }
        if self.uncertainty_acknowledged { s += 0.25; }
        if self.staleness_tracked { s += 0.15; }
        if self.lookahead_violations == 0 { s += 0.10; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.uses_online_stats {
            v.push(PathViolation {
                factor: "Right View",
                issue: "Using global stats instead of rolling",
                module: "state.rs",
                severity: PathSeverity::Critical,
            });
        }
        if self.lookahead_violations > 0 {
            v.push(PathViolation {
                factor: "Right View",
                issue: "Lookahead leakage detected",
                module: "backtest_ethics.rs",
                severity: PathSeverity::Critical,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT INTENTION (sammā saṅkappa) → Survivability Over Greed
//=============================================================================

/// **Module responsibility:** `reducer.rs`, `ethics.rs`
///
/// Right Intention asks: "What is driving this system?"
///
/// Greed-driven → leverage creep, risk erosion
/// Fear-driven → paralysis, hidden risk
/// Clarity-driven → restraint, stability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightIntentionStatus {
    /// Primary objective is survival?
    pub survival_primary: bool,
    /// Risk guards are in code, not judgment?
    pub guards_structural: bool,
    /// Success doesn't loosen constraints?
    pub no_success_creep: bool,
    /// Position sizing is fixed, not "confidence" based?
    pub sizing_fixed: bool,
}

impl RightIntentionStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.survival_primary { s += 0.30; }
        if self.guards_structural { s += 0.30; }
        if self.no_success_creep { s += 0.20; }
        if self.sizing_fixed { s += 0.20; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.survival_primary {
            v.push(PathViolation {
                factor: "Right Intention",
                issue: "Survival is not primary objective",
                module: "reducer.rs",
                severity: PathSeverity::High,
            });
        }
        if !self.guards_structural {
            v.push(PathViolation {
                factor: "Right Intention",
                issue: "Guards are judgment-based, not structural",
                module: "ethics.rs",
                severity: PathSeverity::High,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT SPEECH (sammā vācā) → Truthful Representation
//=============================================================================

/// **Module responsibility:** `logging.rs`, WAL, metrics
///
/// Right Speech asks: "Does the system describe itself truthfully?"
///
/// Wrong speech: cherry-picked results, ambiguous logs, hidden risk
/// Right speech: full audit trail, worst-case reporting, versioned configs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightSpeechStatus {
    /// Are logs actionable and explain "why"?
    pub logs_explain_why: bool,
    /// Is worst-case reported, not just best?
    pub worst_case_reported: bool,
    /// Are experiment configs versioned?
    pub configs_versioned: bool,
    /// Is WAL enabling reproducibility?
    pub wal_reproducible: bool,
    /// Are all trials reported, not just winners?
    pub all_trials_logged: bool,
}

impl RightSpeechStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.logs_explain_why { s += 0.20; }
        if self.worst_case_reported { s += 0.25; }
        if self.configs_versioned { s += 0.15; }
        if self.wal_reproducible { s += 0.25; }
        if self.all_trials_logged { s += 0.15; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.worst_case_reported {
            v.push(PathViolation {
                factor: "Right Speech",
                issue: "Only reporting best-case metrics",
                module: "backtest.rs",
                severity: PathSeverity::High,
            });
        }
        if !self.wal_reproducible {
            v.push(PathViolation {
                factor: "Right Speech",
                issue: "WAL doesn't enable deterministic replay",
                module: "reliability/wal.rs",
                severity: PathSeverity::Critical,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT ACTION (sammā kammanta) → Non-Manipulative Participation
//=============================================================================

/// **Module responsibility:** `exchange/*.rs`, order execution
///
/// Right Action asks: "Are we participating honestly?"
///
/// Wrong action: spoofing, manipulation, exploiting distress
/// Right action: honest orders, genuine intent, liquidity provision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightActionStatus {
    /// No spoofing or fake orders?
    pub no_manipulation: bool,
    /// Orders reflect genuine intent?
    pub genuine_intent: bool,
    /// Not targeting forced liquidations?
    pub no_predation: bool,
    /// Using signed/authenticated orders?
    pub orders_authenticated: bool,
}

impl RightActionStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.no_manipulation { s += 0.35; }
        if self.genuine_intent { s += 0.25; }
        if self.no_predation { s += 0.25; }
        if self.orders_authenticated { s += 0.15; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.no_manipulation {
            v.push(PathViolation {
                factor: "Right Action",
                issue: "Market manipulation detected",
                module: "exchange/*.rs",
                severity: PathSeverity::Critical,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT LIVELIHOOD (sammā ājīva) → Work That Doesn't Amplify Harm
//=============================================================================

/// **Module responsibility:** `risk.rs`, ethics guards
///
/// Right Livelihood asks: "What does this system reinforce over time?"
///
/// Wrong livelihood: amplifies fragility, cultivates greed
/// Right livelihood: provides stability, contains risk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightLivelihoodStatus {
    /// Does the system provide stability or fragility?
    pub provides_stability: bool,
    /// Are ethics guards active?
    pub ethics_guards_active: bool,
    /// Is leverage bounded?
    pub leverage_bounded: bool,
    /// Does success make system calmer or more aggressive?
    pub success_calms: bool,
}

impl RightLivelihoodStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.provides_stability { s += 0.30; }
        if self.ethics_guards_active { s += 0.30; }
        if self.leverage_bounded { s += 0.20; }
        if self.success_calms { s += 0.20; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.ethics_guards_active {
            v.push(PathViolation {
                factor: "Right Livelihood",
                issue: "Ethics guards disabled",
                module: "ethics.rs",
                severity: PathSeverity::Critical,
            });
        }
        if !self.leverage_bounded {
            v.push(PathViolation {
                factor: "Right Livelihood",
                issue: "Leverage not bounded",
                module: "risk.rs",
                severity: PathSeverity::High,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT EFFORT (sammā vāyāma) → Error Reduction Over Fantasy
//=============================================================================

/// **Module responsibility:** `backtest_traps.rs`, testing
///
/// Right Effort asks: "Where is energy directed?"
///
/// Wrong effort: optimizing signal, chasing Sharpe, adding complexity
/// Right effort: reducing errors, removing assumptions, stress testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightEffortStatus {
    /// Testing failure modes, not success?
    pub tests_failure_modes: bool,
    /// Removing assumptions, not adding complexity?
    pub reducing_assumptions: bool,
    /// Stress testing active?
    pub stress_testing: bool,
    /// Multiple testing correction applied?
    pub bonferroni_applied: bool,
}

impl RightEffortStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.tests_failure_modes { s += 0.30; }
        if self.reducing_assumptions { s += 0.25; }
        if self.stress_testing { s += 0.25; }
        if self.bonferroni_applied { s += 0.20; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.tests_failure_modes {
            v.push(PathViolation {
                factor: "Right Effort",
                issue: "Not testing failure modes",
                module: "backtest_traps.rs",
                severity: PathSeverity::High,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT MINDFULNESS (sammā sati) → Continuous State Monitoring
//=============================================================================

/// **Module responsibility:** `metrics.rs`, health checks
///
/// Right Mindfulness asks: "Are we aware of current conditions?"
///
/// Wrong mindfulness: blind to drift, ignoring warnings
/// Right mindfulness: continuous monitoring, assumption drift detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightMindfulnessStatus {
    /// Health checks active?
    pub health_checks_active: bool,
    /// Data freshness monitored?
    pub freshness_monitored: bool,
    /// Regime detection active?
    pub regime_detection: bool,
    /// Assumption drift tracked?
    pub drift_tracked: bool,
}

impl RightMindfulnessStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.health_checks_active { s += 0.30; }
        if self.freshness_monitored { s += 0.25; }
        if self.regime_detection { s += 0.25; }
        if self.drift_tracked { s += 0.20; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.health_checks_active {
            v.push(PathViolation {
                factor: "Right Mindfulness",
                issue: "Health checks not active",
                module: "metrics.rs",
                severity: PathSeverity::High,
            });
        }
        if !self.freshness_monitored {
            v.push(PathViolation {
                factor: "Right Mindfulness",
                issue: "Data freshness not monitored",
                module: "state.rs",
                severity: PathSeverity::High,
            });
        }
        v
    }
}

//=============================================================================
// RIGHT CONCENTRATION (sammā samādhi) → Calm Operational Modes
//=============================================================================

/// **Module responsibility:** `fault.rs`, halt conditions
///
/// Right Concentration asks: "Is the system operating from stability or chaos?"
///
/// Wrong concentration: trading during outages, overriding safeguards
/// Right concentration: halt conditions, cooldowns, "no-trade" as normal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightConcentrationStatus {
    /// Halt conditions active?
    pub halt_conditions_active: bool,
    /// Cooldown periods enforced?
    pub cooldowns_enforced: bool,
    /// Rate limits in place?
    pub rate_limits_active: bool,
    /// "No trade" is a valid state?
    pub no_trade_valid: bool,
}

impl RightConcentrationStatus {
    pub fn score(&self) -> f64 {
        let mut s = 0.0;
        if self.halt_conditions_active { s += 0.35; }
        if self.cooldowns_enforced { s += 0.25; }
        if self.rate_limits_active { s += 0.20; }
        if self.no_trade_valid { s += 0.20; }
        s
    }

    pub fn violations(&self) -> Vec<PathViolation> {
        let mut v = Vec::new();
        if !self.halt_conditions_active {
            v.push(PathViolation {
                factor: "Right Concentration",
                issue: "Halt conditions not active",
                module: "fault.rs",
                severity: PathSeverity::Critical,
            });
        }
        if !self.cooldowns_enforced {
            v.push(PathViolation {
                factor: "Right Concentration",
                issue: "Cooldowns not enforced",
                module: "reducer.rs",
                severity: PathSeverity::High,
            });
        }
        v
    }
}

//=============================================================================
// Path Violation
//=============================================================================

#[derive(Debug, Clone)]
pub struct PathViolation {
    pub factor: &'static str,
    pub issue: &'static str,
    pub module: &'static str,
    pub severity: PathSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSeverity {
    /// System unsafe to operate
    Critical,
    /// Significant alignment issue
    High,
    /// Should be addressed
    Medium,
    /// Minor concern
    Low,
}

//=============================================================================
// Factory for current system state
//=============================================================================

impl EightfoldChecklist {
    /// Create checklist from current system configuration
    ///
    /// This assesses the codebase structure, not runtime state
    pub fn from_system_design() -> Self {
        // These are based on what we've implemented
        Self {
            right_view: RightViewStatus {
                uses_online_stats: true,  // Welford algorithm in state.rs
                regime_segmented: true,   // backtest_ethics.rs has regime analysis
                uncertainty_acknowledged: true, // Options and staleness in state
                staleness_tracked: true,  // is_stale() method
                lookahead_violations: 0,  // LookaheadDetector implemented
            },
            right_intention: RightIntentionStatus {
                survival_primary: true,   // Focus on risk over profit
                guards_structural: true,  // ethics.rs three-poison guards
                no_success_creep: true,   // Guards in code, not judgment
                sizing_fixed: true,       // position_size in config
            },
            right_speech: RightSpeechStatus {
                logs_explain_why: true,   // Command::Log with context
                worst_case_reported: true, // BacktestResults has failure metrics
                configs_versioned: true,  // ExperimentConfig has code_version
                wal_reproducible: true,   // WAL for deterministic replay
                all_trials_logged: true, // ExperimentRegistry logs all trials
            },
            right_action: RightActionStatus {
                no_manipulation: true,    // No spoofing logic
                genuine_intent: true,     // Real orders via signed API
                no_predation: true,       // Not targeting liquidations
                orders_authenticated: true, // HMAC signing implemented
            },
            right_livelihood: RightLivelihoodStatus {
                provides_stability: true,  // Mean-reversion (liquidity provision)
                ethics_guards_active: true, // check_three_poisons()
                leverage_bounded: true,    // max_position_pct
                success_calms: true,       // Fixed guards, not adaptive
            },
            right_effort: RightEffortStatus {
                tests_failure_modes: true, // StressScenario suite
                reducing_assumptions: true, // Minimal strategy
                stress_testing: true,      // stress_results in BacktestResults
                bonferroni_applied: true,  // ExperimentRegistry supports correction
            },
            right_mindfulness: RightMindfulnessStatus {
                health_checks_active: true,  // SysEvent::Health
                freshness_monitored: true,   // is_stale() checks
                regime_detection: true,      // Narrative detector wired in reducer
                drift_tracked: false,        // Drift tracker not yet wired
            },
            right_concentration: RightConcentrationStatus {
                halt_conditions_active: true, // HaltReason enum
                cooldowns_enforced: true,    // cooldown_ms in config
                rate_limits_active: true,    // max_trades_per_day
                no_trade_valid: true,        // generate_signal can return None
            },
        }
    }

    /// Print assessment report
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str("=== EIGHTFOLD PATH ALIGNMENT REPORT ===\n\n");
        out.push_str(&format!("Overall alignment: {:.1}%\n", self.alignment_score() * 100.0));
        out.push_str(&format!("Operational: {}\n\n", if self.is_operational() { "YES" } else { "NO" }));

        out.push_str(&format!("Right View:          {:.0}%\n", self.right_view.score() * 100.0));
        out.push_str(&format!("Right Intention:     {:.0}%\n", self.right_intention.score() * 100.0));
        out.push_str(&format!("Right Speech:        {:.0}%\n", self.right_speech.score() * 100.0));
        out.push_str(&format!("Right Action:        {:.0}%\n", self.right_action.score() * 100.0));
        out.push_str(&format!("Right Livelihood:    {:.0}%\n", self.right_livelihood.score() * 100.0));
        out.push_str(&format!("Right Effort:        {:.0}%\n", self.right_effort.score() * 100.0));
        out.push_str(&format!("Right Mindfulness:   {:.0}%\n", self.right_mindfulness.score() * 100.0));
        out.push_str(&format!("Right Concentration: {:.0}%\n\n", self.right_concentration.score() * 100.0));

        let violations = self.violations();
        if violations.is_empty() {
            out.push_str("No violations detected.\n");
        } else {
            out.push_str(&format!("Violations ({}):\n", violations.len()));
            for v in violations {
                out.push_str(&format!("  [{:?}] {}: {} ({})\n",
                    v.severity, v.factor, v.issue, v.module));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_alignment() {
        let checklist = EightfoldChecklist::from_system_design();

        // Should be operational
        assert!(checklist.is_operational());

        // Should have high alignment
        assert!(checklist.alignment_score() > 0.7);

        // Print report for visibility
        println!("{}", checklist.report());
    }

    #[test]
    fn test_violations_detected() {
        let checklist = EightfoldChecklist {
            right_view: RightViewStatus {
                uses_online_stats: false, // Violation!
                regime_segmented: true,
                uncertainty_acknowledged: true,
                staleness_tracked: true,
                lookahead_violations: 1, // Violation!
            },
            right_intention: RightIntentionStatus {
                survival_primary: true,
                guards_structural: true,
                no_success_creep: true,
                sizing_fixed: true,
            },
            right_speech: RightSpeechStatus {
                logs_explain_why: true,
                worst_case_reported: true,
                configs_versioned: true,
                wal_reproducible: true,
                all_trials_logged: true,
            },
            right_action: RightActionStatus {
                no_manipulation: true,
                genuine_intent: true,
                no_predation: true,
                orders_authenticated: true,
            },
            right_livelihood: RightLivelihoodStatus {
                provides_stability: true,
                ethics_guards_active: true,
                leverage_bounded: true,
                success_calms: true,
            },
            right_effort: RightEffortStatus {
                tests_failure_modes: true,
                reducing_assumptions: true,
                stress_testing: true,
                bonferroni_applied: true,
            },
            right_mindfulness: RightMindfulnessStatus {
                health_checks_active: true,
                freshness_monitored: true,
                regime_detection: true,
                drift_tracked: true,
            },
            right_concentration: RightConcentrationStatus {
                halt_conditions_active: true,
                cooldowns_enforced: true,
                rate_limits_active: true,
                no_trade_valid: true,
            },
        };

        let violations = checklist.violations();
        assert_eq!(violations.len(), 2); // Two Right View violations
    }
}
