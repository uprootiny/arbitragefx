//! Backtest Trap Checklist - mapped to actual codebase modules
//!
//! Each trap is a form of delusion that makes backtests lie.
//! This module provides guards and checks.
//!
//! ## Usage
//!
//! Run `BacktestIntegrity::check_all()` before trusting any backtest result.

/// Backtest integrity checker
pub struct BacktestIntegrity {
    violations: Vec<TrapViolation>,
}

#[derive(Debug, Clone)]
pub struct TrapViolation {
    pub trap_id: u8,
    pub trap_name: &'static str,
    pub severity: Severity,
    pub description: String,
    pub module_location: &'static str,
    pub guard_recommendation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Severity {
    /// Results are invalid
    Critical,
    /// Results are unreliable
    High,
    /// Results may be optimistic
    Medium,
    /// Minor concern
    Low,
}

impl BacktestIntegrity {
    pub fn new() -> Self {
        Self {
            violations: Vec::new(),
        }
    }

    /// Add a violation
    pub fn add_violation(&mut self, v: TrapViolation) {
        self.violations.push(v);
    }

    /// Check if backtest can be trusted
    pub fn is_trustworthy(&self) -> bool {
        !self
            .violations
            .iter()
            .any(|v| v.severity == Severity::Critical)
    }

    /// Get all violations
    pub fn violations(&self) -> &[TrapViolation] {
        &self.violations
    }

    /// Summary report
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str("=== BACKTEST INTEGRITY REPORT ===\n\n");

        let critical = self
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Critical)
            .count();
        let high = self
            .violations
            .iter()
            .filter(|v| v.severity == Severity::High)
            .count();

        out.push_str(&format!("Critical violations: {}\n", critical));
        out.push_str(&format!("High violations: {}\n", high));
        out.push_str(&format!(
            "Trustworthy: {}\n\n",
            if self.is_trustworthy() { "YES" } else { "NO" }
        ));

        for v in &self.violations {
            out.push_str(&format!(
                "[{:?}] Trap #{}: {}\n  Module: {}\n  Issue: {}\n  Guard: {}\n\n",
                v.severity,
                v.trap_id,
                v.trap_name,
                v.module_location,
                v.description,
                v.guard_recommendation
            ));
        }

        out
    }
}

impl Default for BacktestIntegrity {
    fn default() -> Self {
        Self::new()
    }
}

//=============================================================================
// TRAP DEFINITIONS - mapped to your codebase
//=============================================================================

/// Trap #1: Close-price omniscience
///
/// **Location:** `engine_backtest.rs:71-75` and `reducer.rs:generate_signal()`
///
/// **The Lie:** Using candle close to generate signal, then executing "at close"
///
/// **Guard Location:** `engine_backtest.rs` - add execution delay
pub mod trap_01_close_omniscience {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 1,
        name: "Close-price omniscience",
        severity: Severity::Critical,
        module: "src/bin/engine_backtest.rs:71-75",
        guard: "Execute at NEXT bar open, not current close. Add delay_bars=1 to order execution.",
    };

    /// Check if execution happens after signal bar
    pub fn check(signal_bar_ts: u64, execution_ts: u64) -> Option<TrapViolation> {
        if execution_ts <= signal_bar_ts {
            Some(TrapViolation {
                trap_id: TRAP.id,
                trap_name: TRAP.name,
                severity: TRAP.severity,
                description: format!(
                    "Signal at {} executed at {} (same or earlier bar)",
                    signal_bar_ts, execution_ts
                ),
                module_location: TRAP.module,
                guard_recommendation: TRAP.guard,
            })
        } else {
            None
        }
    }
}

/// Trap #2: Indicator warm-up pollution
///
/// **Location:** `engine/state.rs:on_candle()` and `reducer.rs:generate_signal()`
///
/// **The Lie:** Trading before EMA/variance windows are fully populated
///
/// **Guard Location:** `generate_signal()` already checks `candle_count < 10`
pub mod trap_02_warmup_pollution {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 2,
        name: "Indicator warm-up pollution",
        severity: Severity::High,
        module: "src/engine/state.rs:on_candle(), reducer.rs:generate_signal()",
        guard: "Require candle_count >= max(EMA_slow_period, variance_window) before any signal",
    };

    /// Check warm-up requirements
    pub fn check(
        candle_count: u64,
        ema_slow_period: u64,
        variance_min: u64,
    ) -> Option<TrapViolation> {
        let required = ema_slow_period.max(variance_min);
        if candle_count < required {
            Some(TrapViolation {
                trap_id: TRAP.id,
                trap_name: TRAP.name,
                severity: TRAP.severity,
                description: format!(
                    "Trading with {} candles but need {} for warm-up",
                    candle_count, required
                ),
                module_location: TRAP.module,
                guard_recommendation: TRAP.guard,
            })
        } else {
            None
        }
    }
}

/// Trap #3: Global normalization lookahead
///
/// **Location:** `engine/state.rs:z_momentum()`, `z_mean_deviation()`
///
/// **The Lie:** Using full-dataset mean/std instead of rolling
///
/// **Guard:** Already using Welford online algorithm - GOOD
pub mod trap_03_global_normalization {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 3,
        name: "Global normalization lookahead",
        severity: Severity::Critical,
        module: "src/engine/state.rs:z_momentum(), z_mean_deviation()",
        guard: "Use Welford online algorithm (already implemented). Never import 'global' stats.",
    };

    /// This trap is guarded by design (Welford algorithm)
    /// This check ensures no global stats are used
    pub fn check_uses_online_stats(uses_welford: bool) -> Option<TrapViolation> {
        if !uses_welford {
            Some(TrapViolation {
                trap_id: TRAP.id,
                trap_name: TRAP.name,
                severity: TRAP.severity,
                description: "Not using online statistics algorithm".to_string(),
                module_location: TRAP.module,
                guard_recommendation: TRAP.guard,
            })
        } else {
            None
        }
    }
}

/// Trap #4: Fixed fee/slippage
///
/// **Location:** `engine_backtest.rs:116-121`
///
/// **The Lie:** Flat 0.1% fee ignores volatility-dependent execution costs
///
/// **Guard:** Make slippage a function of volatility
pub mod trap_04_fixed_slippage {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 4,
        name: "Fixed fee/slippage model",
        severity: Severity::Medium,
        module: "src/bin/engine_backtest.rs:116-121",
        guard:
            "slippage = base_slippage * (1 + volatility_multiplier * current_vol / baseline_vol)",
    };

    /// Calculate volatility-adjusted slippage
    pub fn adjusted_slippage(base_slippage: f64, current_vol: f64, baseline_vol: f64) -> f64 {
        let vol_mult = if baseline_vol > 0.0 {
            (current_vol / baseline_vol).max(1.0)
        } else {
            1.0
        };
        base_slippage * vol_mult
    }
}

/// Trap #5: Perfect fill assumption
///
/// **Location:** `engine_backtest.rs:123-133` (immediate full fill)
///
/// **The Lie:** Every order fills completely at intended price
///
/// **Guard:** Use partial fill simulation in fuzz.rs
pub mod trap_05_perfect_fill {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 5,
        name: "Perfect fill assumption",
        severity: Severity::High,
        module: "src/bin/engine_backtest.rs:123-133, fuzz.rs",
        guard:
            "Simulate partial fills, timeouts, and adverse selection. Hook fuzz.rs into backtest.",
    };
}

/// Trap #6: Simplified backtest order model
///
/// **Location:** Backtest uses direct position updates vs live order state machine
///
/// **The Lie:** Validating wrong system
///
/// **Guard:** Already using same reducer - GOOD
pub mod trap_06_order_model_divergence {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 6,
        name: "Order state machine not used in backtest",
        severity: Severity::Critical,
        module: "src/bin/engine_backtest.rs vs src/engine/reducer.rs",
        guard: "Run backtest through same reducer as live. Already implemented!",
    };
}

/// Trap #7: Out-of-order event blindness
///
/// **Location:** `engine_backtest.rs` feeds events in order
///
/// **The Lie:** Real events arrive out of order
///
/// **Guard:** Hook permute.rs into backtest feed
pub mod trap_07_event_order {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 7,
        name: "Out-of-order event blindness",
        severity: Severity::Medium,
        module: "src/bin/engine_backtest.rs, permute.rs",
        guard: "Add scrambler layer that permutes arrival order within bounded window",
    };

    /// Scramble events within a time window
    pub fn scramble_events<T: Clone>(
        events: &[(u64, T)],
        window_ms: u64,
        seed: u64,
    ) -> Vec<(u64, T)> {
        use std::collections::BTreeMap;

        // Group by window
        let mut windows: BTreeMap<u64, Vec<(u64, T)>> = BTreeMap::new();
        for (ts, event) in events {
            let window_key = ts / window_ms;
            windows
                .entry(window_key)
                .or_default()
                .push((*ts, event.clone()));
        }

        // Shuffle within each window using simple LCG
        let mut result = Vec::new();
        let mut rng = seed;
        for (_key, mut group) in windows {
            // Fisher-Yates shuffle with LCG
            for i in (1..group.len()).rev() {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = (rng as usize) % (i + 1);
                group.swap(i, j);
            }
            result.extend(group);
        }

        result
    }
}

/// Trap #8: Bar time vs arrival time conflation
///
/// **Location:** `engine_backtest.rs:72` uses candle ts directly
///
/// **The Lie:** You learn the candle instantly
///
/// **Guard:** Add observed_at with delay model
pub mod trap_08_arrival_time {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 8,
        name: "Bar time vs arrival time conflation",
        severity: Severity::Medium,
        module: "src/bin/engine_backtest.rs:72, src/engine/events.rs",
        guard: "Add observed_at field to events; trade logic uses observed_at, not bar close time",
    };

    /// Model arrival delay
    pub fn model_arrival_delay(bar_close_ts: u64, base_delay_ms: u64, volatility_mult: f64) -> u64 {
        let delay = (base_delay_ms as f64 * volatility_mult) as u64;
        bar_close_ts + delay
    }
}

/// Trap #9: Aux data clock misalignment
///
/// **Location:** `feed/aux_data.rs`, `engine_backtest.rs:77-96`
///
/// **The Lie:** Aux data aligned by nearest timestamp without publication delay
///
/// **Guard:** Only use aux where published_at <= decision_time
pub mod trap_09_aux_clock {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 9,
        name: "Aux data clock misalignment",
        severity: Severity::High,
        module: "src/feed/aux_data.rs, src/bin/engine_backtest.rs:77-96",
        guard: "Track published_at for all aux data; only use where published_at <= decision_time",
    };
}

/// Trap #10: Staleness as zero
///
/// **Location:** `feed/aux_data.rs:62` defaults to empty struct
///
/// **The Lie:** Missing data = 0 instead of unknown
///
/// **Guard:** Use Option<T> with explicit staleness TTL
pub mod trap_10_staleness_zero {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 10,
        name: "Staleness treated as zero",
        severity: Severity::High,
        module: "src/feed/aux_data.rs:62, src/engine/state.rs",
        guard: "Use Option<T> for aux data; halt or reduce exposure when stale",
    };

    /// Check if value should be considered stale
    pub fn is_stale(last_update_ts: u64, now: u64, ttl_ms: u64) -> bool {
        now.saturating_sub(last_update_ts) > ttl_ms
    }
}

/// Trap #11: Selection bias
///
/// **Location:** `data/*.csv`, `trials.rs` symbol/period choices
///
/// **The Lie:** Picking winning periods/symbols
///
/// **Guard:** Pre-register test sets; always report all runs
pub mod trap_11_selection_bias {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 11,
        name: "Selection bias via start date/symbol choice",
        severity: Severity::High,
        module: "src/backtest/trials.rs, data/*.csv",
        guard: "Pre-register test periods in config; report ALL runs; include bad regimes",
    };

    /// Required test regimes
    pub fn required_regimes() -> Vec<&'static str> {
        vec![
            "low_volatility",
            "high_volatility",
            "trending_up",
            "trending_down",
            "ranging",
            "crisis", // Must include at least one crisis period
        ]
    }
}

/// Trap #12: Multiple testing without penalty
///
/// **Location:** `backtest/trials.rs`
///
/// **The Lie:** Best of N runs is meaningful
///
/// **Guard:** Walk-forward + report distribution
pub mod trap_12_multiple_testing {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 12,
        name: "Multiple testing without penalty",
        severity: Severity::Critical,
        module: "src/backtest/trials.rs",
        guard: "Walk-forward validation; report distribution not max; Bonferroni correction",
    };

    /// Apply Bonferroni correction
    pub fn bonferroni_alpha(base_alpha: f64, num_trials: u32) -> f64 {
        base_alpha / num_trials as f64
    }
}

/// Trap #13: Metrics hiding tail risk
///
/// **Location:** `backtest.rs` metrics
///
/// **The Lie:** Sharpe/win-rate hide ruin
///
/// **Guard:** First-class: max_dd, CVaR, worst-day
pub mod trap_13_tail_risk_hidden {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 13,
        name: "Metrics that hide tail risk",
        severity: Severity::High,
        module: "src/backtest.rs, src/engine/backtest_ethics.rs",
        guard:
            "Primary metrics: max_drawdown, CVaR, worst_day, underwater_time. Sharpe is secondary.",
    };

    /// Required primary metrics (must be reported)
    pub fn required_metrics() -> Vec<&'static str> {
        vec![
            "max_drawdown_pct",
            "max_drawdown_duration",
            "worst_trade_pct",
            "loss_99th_percentile",
            "consecutive_loss_max",
        ]
    }
}

/// Trap #14: Regime collapse masked
///
/// **Location:** `backtest.rs` single aggregate line
///
/// **The Lie:** One number hides regime dependence
///
/// **Guard:** Segment by regime; require acceptable across segments
pub mod trap_14_regime_masked {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 14,
        name: "Regime collapse masked by aggregation",
        severity: Severity::High,
        module: "src/backtest.rs, src/engine/backtest_ethics.rs",
        guard: "Segment metrics by vol/trend/funding regime; flag if any segment catastrophic",
    };
}

/// Trap #15: Clean-room vs adversarial
///
/// **Location:** Backtest has no infra failures
///
/// **The Lie:** Live is adversarial
///
/// **Guard:** Chaos knobs: random failures, delays, duplicates
pub mod trap_15_clean_room {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 15,
        name: "Backtest clean-room vs live adversarial",
        severity: Severity::High,
        module: "src/bin/engine_backtest.rs, src/fault.rs",
        guard: "Add chaos knobs: fetch_failure_rate, data_delay, duplicate_events, missing_candles",
    };

    /// Chaos configuration
    #[derive(Debug, Clone)]
    pub struct ChaosConfig {
        /// Probability of fetch failure (0.0 - 1.0)
        pub fetch_failure_rate: f64,
        /// Additional delay range in ms
        pub delay_range_ms: (u64, u64),
        /// Probability of duplicate event
        pub duplicate_rate: f64,
        /// Probability of missing candle
        pub missing_candle_rate: f64,
    }

    impl Default for ChaosConfig {
        fn default() -> Self {
            Self {
                fetch_failure_rate: 0.01,
                delay_range_ms: (0, 1000),
                duplicate_rate: 0.005,
                missing_candle_rate: 0.01,
            }
        }
    }
}

/// Trap #16: WAL not used for determinism
///
/// **Location:** `reliability/wal.rs`
///
/// **The Lie:** WAL is logging, not replay basis
///
/// **Guard:** Test: run → hash → crash → replay → same hash
pub mod trap_16_wal_determinism {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 16,
        name: "WAL not used for determinism",
        severity: Severity::Critical,
        module: "src/reliability/wal.rs",
        guard: "CI test: run → snapshot_hash → crash → replay_wal → assert_same_hash",
    };

    /// Verify replay determinism
    pub fn verify_replay_determinism(
        original_hash: u64,
        replayed_hash: u64,
    ) -> Option<TrapViolation> {
        if original_hash != replayed_hash {
            Some(TrapViolation {
                trap_id: TRAP.id,
                trap_name: TRAP.name,
                severity: TRAP.severity,
                description: format!(
                    "Replay diverged: original={} replayed={}",
                    original_hash, replayed_hash
                ),
                module_location: TRAP.module,
                guard_recommendation: TRAP.guard,
            })
        } else {
            None
        }
    }
}

/// Trap #17: Paper/live code divergence
///
/// **Location:** Adapter differences
///
/// **The Lie:** Validating wrong machine
///
/// **Guard:** Single code path; adapters differ only in transport
pub mod trap_17_code_divergence {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 17,
        name: "Paper/live code path divergence",
        severity: Severity::Critical,
        module: "src/adapter/*.rs, src/bin/engine_*.rs",
        guard: "Single reducer/risk/order code path. Adapters differ ONLY in HTTP transport.",
    };
}

/// Trap #18: Rounding/min-notional ignored
///
/// **Location:** Backtest uses continuous sizes
///
/// **The Lie:** Real exchanges have step sizes
///
/// **Guard:** Apply exchange filters in simulation
pub mod trap_18_rounding {
    use super::*;

    pub const TRAP: TrapDef = TrapDef {
        id: 18,
        name: "Rounding and min-notional ignored",
        severity: Severity::Medium,
        module: "src/bin/engine_backtest.rs, src/exchange/*.rs",
        guard: "Apply tick_size, step_size, min_notional in backtest same as live",
    };

    /// Exchange filters
    #[derive(Debug, Clone)]
    pub struct ExchangeFilters {
        /// Price tick size (e.g., 0.01)
        pub tick_size: f64,
        /// Quantity step size (e.g., 0.001)
        pub step_size: f64,
        /// Minimum notional value (e.g., 10.0)
        pub min_notional: f64,
    }

    impl ExchangeFilters {
        pub fn binance_btcusdt() -> Self {
            Self {
                tick_size: 0.01,
                step_size: 0.00001,
                min_notional: 10.0,
            }
        }

        /// Round quantity to step size
        pub fn round_qty(&self, qty: f64) -> f64 {
            (qty / self.step_size).floor() * self.step_size
        }

        /// Round price to tick size
        pub fn round_price(&self, price: f64) -> f64 {
            (price / self.tick_size).round() * self.tick_size
        }

        /// Check if order meets minimum notional
        pub fn meets_min_notional(&self, qty: f64, price: f64) -> bool {
            qty * price >= self.min_notional
        }
    }
}

/// Trap definition
pub struct TrapDef {
    pub id: u8,
    pub name: &'static str,
    pub severity: Severity,
    pub module: &'static str,
    pub guard: &'static str,
}

/// Get all trap definitions
pub fn all_traps() -> Vec<&'static TrapDef> {
    vec![
        &trap_01_close_omniscience::TRAP,
        &trap_02_warmup_pollution::TRAP,
        &trap_03_global_normalization::TRAP,
        &trap_04_fixed_slippage::TRAP,
        &trap_05_perfect_fill::TRAP,
        &trap_06_order_model_divergence::TRAP,
        &trap_07_event_order::TRAP,
        &trap_08_arrival_time::TRAP,
        &trap_09_aux_clock::TRAP,
        &trap_10_staleness_zero::TRAP,
        &trap_11_selection_bias::TRAP,
        &trap_12_multiple_testing::TRAP,
        &trap_13_tail_risk_hidden::TRAP,
        &trap_14_regime_masked::TRAP,
        &trap_15_clean_room::TRAP,
        &trap_16_wal_determinism::TRAP,
        &trap_17_code_divergence::TRAP,
        &trap_18_rounding::TRAP,
    ]
}

/// Print checklist for manual review
pub fn print_checklist() {
    println!("=== BACKTEST INTEGRITY CHECKLIST ===\n");
    println!("Review each trap against your implementation:\n");

    for trap in all_traps() {
        println!("[ ] #{:02} {} [{:?}]", trap.id, trap.name, trap.severity);
        println!("    Module: {}", trap.module);
        println!("    Guard: {}\n", trap.guard);
    }
}

/// Guard status for a backtest trap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GuardStatus {
    Guarded,
    Partial,
    Unguarded,
}

/// Status of a backtest trap with its guard implementation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrapStatus {
    pub id: u8,
    pub name: &'static str,
    pub severity: Severity,
    pub guard: GuardStatus,
    pub evidence: &'static str,
}

/// Returns the current guard status for all 18 backtest traps.
pub fn trap_status() -> Vec<TrapStatus> {
    vec![
        TrapStatus {
            id: 1,
            name: "Close omniscience",
            severity: Severity::Critical,
            guard: GuardStatus::Partial,
            evidence: "Latency model delays execution; but latency_min can be 0",
        },
        TrapStatus {
            id: 2,
            name: "Warmup pollution",
            severity: Severity::High,
            guard: GuardStatus::Guarded,
            evidence: "FeaturePipeline pre-fills 200 candles before trading",
        },
        TrapStatus {
            id: 3,
            name: "Global normalization",
            severity: Severity::Critical,
            guard: GuardStatus::Guarded,
            evidence: "Welford online algorithm, no lookahead",
        },
        TrapStatus {
            id: 4,
            name: "Fixed slippage",
            severity: Severity::Medium,
            guard: GuardStatus::Guarded,
            evidence: "Volatility-multiplied slippage model in ExecConfig",
        },
        TrapStatus {
            id: 5,
            name: "Perfect fill assumption",
            severity: Severity::High,
            guard: GuardStatus::Partial,
            evidence: "Partial fill sim exists; fill_prob model simplified",
        },
        TrapStatus {
            id: 6,
            name: "Order model divergence",
            severity: Severity::Critical,
            guard: GuardStatus::Guarded,
            evidence: "Same reducer for backtest and live",
        },
        TrapStatus {
            id: 7,
            name: "Out-of-order events",
            severity: Severity::Medium,
            guard: GuardStatus::Unguarded,
            evidence: "No scrambler layer in backtest",
        },
        TrapStatus {
            id: 8,
            name: "Arrival time conflation",
            severity: Severity::Medium,
            guard: GuardStatus::Unguarded,
            evidence: "No observed_at delay model",
        },
        TrapStatus {
            id: 9,
            name: "Aux data clock misalign",
            severity: Severity::High,
            guard: GuardStatus::Partial,
            evidence: "TTL check exists but no published_at tracking",
        },
        TrapStatus {
            id: 10,
            name: "Staleness as zero",
            severity: Severity::High,
            guard: GuardStatus::Guarded,
            evidence: "TTL-based staleness detection in data module",
        },
        TrapStatus {
            id: 11,
            name: "Selection bias",
            severity: Severity::High,
            guard: GuardStatus::Unguarded,
            evidence: "No pre-registration; user controls dataset choice",
        },
        TrapStatus {
            id: 12,
            name: "Multiple testing",
            severity: Severity::Critical,
            guard: GuardStatus::Guarded,
            evidence: "Bonferroni correction in walk_forward",
        },
        TrapStatus {
            id: 13,
            name: "Tail risk hidden",
            severity: Severity::High,
            guard: GuardStatus::Partial,
            evidence: "Max drawdown tracked; CVaR not reported",
        },
        TrapStatus {
            id: 14,
            name: "Regime collapse masked",
            severity: Severity::High,
            guard: GuardStatus::Guarded,
            evidence: "regime.rs classifies datasets; bench tests all 4 regimes",
        },
        TrapStatus {
            id: 15,
            name: "Clean room vs adversarial",
            severity: Severity::High,
            guard: GuardStatus::Unguarded,
            evidence: "No chaos injection in backtest",
        },
        TrapStatus {
            id: 16,
            name: "WAL determinism",
            severity: Severity::Critical,
            guard: GuardStatus::Guarded,
            evidence: "WAL replay tested; determinism assertions in place",
        },
        TrapStatus {
            id: 17,
            name: "Paper/live code divergence",
            severity: Severity::Critical,
            guard: GuardStatus::Guarded,
            evidence: "Single code path; adapters differ only in HTTP",
        },
        TrapStatus {
            id: 18,
            name: "Rounding/min-notional",
            severity: Severity::Medium,
            guard: GuardStatus::Unguarded,
            evidence: "Backtest uses continuous sizes; no exchange filters",
        },
    ]
}

/// Compute integrity score as (guarded_count, total).
pub fn integrity_score() -> (usize, usize) {
    let traps = trap_status();
    let guarded = traps
        .iter()
        .filter(|t| t.guard == GuardStatus::Guarded)
        .count();
    (guarded, traps.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_close_omniscience_check() {
        // Signal at t=100, execute at t=100 = violation
        assert!(trap_01_close_omniscience::check(100, 100).is_some());

        // Signal at t=100, execute at t=200 = ok
        assert!(trap_01_close_omniscience::check(100, 200).is_none());
    }

    #[test]
    fn test_warmup_check() {
        // 5 candles but need 24 for EMA = violation
        assert!(trap_02_warmup_pollution::check(5, 24, 10).is_some());

        // 30 candles, need 24 = ok
        assert!(trap_02_warmup_pollution::check(30, 24, 10).is_none());
    }

    #[test]
    fn test_event_scrambling() {
        let events = vec![
            (100u64, "a"),
            (101, "b"),
            (102, "c"),
            (200, "d"),
            (201, "e"),
        ];

        let scrambled = trap_07_event_order::scramble_events(&events, 50, 42);

        // Should have same events
        assert_eq!(scrambled.len(), events.len());

        // Events within same window may be reordered
        // Events across windows maintain window order
    }

    #[test]
    fn test_exchange_filters() {
        let filters = trap_18_rounding::ExchangeFilters::binance_btcusdt();

        assert_eq!(filters.round_qty(0.123456), 0.12345);
        assert_eq!(filters.round_price(42100.567), 42100.57);
        assert!(filters.meets_min_notional(0.001, 50000.0)); // $50 > $10
        assert!(!filters.meets_min_notional(0.0001, 50000.0)); // $5 < $10
    }

    #[test]
    fn test_wal_determinism() {
        // Same hash = ok
        assert!(trap_16_wal_determinism::verify_replay_determinism(12345, 12345).is_none());

        // Different hash = violation
        assert!(trap_16_wal_determinism::verify_replay_determinism(12345, 12346).is_some());
    }
}
