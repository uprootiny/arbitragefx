//! Ethical guards mapped to the Three Poisons (Buddhism)
//!
//! This module formalizes the relationship between Buddhist ethics
//! and trading system design. The three poisons (kleshas) are:
//!
//! 1. **Greed** (lobha) - attachment to gains, over-extension
//! 2. **Aversion** (dvesha) - fear-driven reactions, revenge trading
//! 3. **Delusion** (moha) - false beliefs, chasing lagging signals
//!
//! Each poison maps to specific failure modes in trading systems,
//! and each has corresponding guards.

use super::state::EngineState;
use super::reducer::ReducerConfig;

/// Result of ethics check
#[derive(Debug, Clone, PartialEq)]
pub enum EthicsViolation {
    /// Greed: position too large relative to equity
    GreedOverExtension { exposure_pct: f64, limit: f64 },
    /// Greed: too many trades (compulsive activity)
    GreedOverTrading { trades: u32, limit: u32 },
    /// Aversion: trading during cooldown (revenge trading)
    AversionRevenge { ms_since_loss: u64, cooldown: u64 },
    /// Aversion: consecutive losses without pause
    AversionCascade { consecutive: u32 },
    /// Delusion: insufficient data for signal validity
    DelusionInsufficient { candles: u64, minimum: u64 },
    /// Delusion: data too stale to trust
    DelusionStale { age_ms: u64, max_age: u64 },
}

/// Check state against all three-poison guards
///
/// Returns `None` if state is aligned, `Some(violation)` if a guard is tripped.
///
/// # Philosophy
///
/// This function operationalizes Right Action by checking:
/// - Are we extending beyond our means? (greed)
/// - Are we reacting to fear? (aversion)
/// - Are we acting on false information? (delusion)
pub fn check_three_poisons(
    state: &EngineState,
    symbol: &str,
    cfg: &ReducerConfig,
) -> Option<EthicsViolation> {
    // === GREED GUARDS ===

    // G1: Position size relative to equity
    let prices: std::collections::HashMap<String, f64> = state.symbols.iter()
        .map(|(k, v)| (k.clone(), v.last_price))
        .collect();
    let exposure = state.portfolio.total_exposure(&prices);
    let exposure_pct = if state.portfolio.equity > 0.0 {
        exposure / state.portfolio.equity
    } else {
        0.0
    };

    if exposure_pct > cfg.max_position_pct {
        return Some(EthicsViolation::GreedOverExtension {
            exposure_pct,
            limit: cfg.max_position_pct,
        });
    }

    // G2: Trade count (compulsive over-trading)
    if state.risk.trades_today >= cfg.max_trades_per_day {
        return Some(EthicsViolation::GreedOverTrading {
            trades: state.risk.trades_today,
            limit: cfg.max_trades_per_day,
        });
    }

    // === AVERSION GUARDS ===

    // A1: Cooldown after loss (prevent revenge trading)
    let ms_since_loss = state.now.saturating_sub(state.risk.last_loss_ts);
    if ms_since_loss < cfg.cooldown_ms && state.risk.last_loss_ts > 0 {
        return Some(EthicsViolation::AversionRevenge {
            ms_since_loss,
            cooldown: cfg.cooldown_ms,
        });
    }

    // A2: Consecutive losses cascade
    if state.risk.consecutive_losses >= 3 {
        return Some(EthicsViolation::AversionCascade {
            consecutive: state.risk.consecutive_losses,
        });
    }

    // === DELUSION GUARDS ===

    if let Some(sym) = state.symbols.get(symbol) {
        // D1: Minimum data for signal validity
        let minimum_candles = 10u64;
        if sym.candle_count < minimum_candles {
            return Some(EthicsViolation::DelusionInsufficient {
                candles: sym.candle_count,
                minimum: minimum_candles,
            });
        }

        // D2: Data freshness
        let data_age = state.now.saturating_sub(sym.last_ts);
        if data_age > cfg.data_stale_ms {
            return Some(EthicsViolation::DelusionStale {
                age_ms: data_age,
                max_age: cfg.data_stale_ms,
            });
        }
    }

    None // All guards pass - action is aligned
}

/// The three-poison framework as documentation
///
/// This maps each poison to:
/// - Its manifestation in trading
/// - The guard that prevents it
/// - The healthy alternative
pub mod framework {
    //! # Greed (Lobha)
    //!
    //! ## Manifestation
    //! - Wanting larger positions after wins
    //! - Taking trades just to be "in the market"
    //! - Loosening risk limits after good runs
    //!
    //! ## Guards
    //! - `max_position_pct`: Hard limit on exposure
    //! - `max_trades_per_day`: Prevents compulsive trading
    //! - `position_size`: Fixed, not scaled by "confidence"
    //!
    //! ## Healthy Alternative
    //! Accept that most of the time, no position is correct.
    //! Size by risk, not by desire for gain.

    //! # Aversion (Dvesha)
    //!
    //! ## Manifestation
    //! - Revenge trading after losses
    //! - Panic exits on normal volatility
    //! - Tightening stops irrationally after drawdown
    //!
    //! ## Guards
    //! - `cooldown_ms`: Forced pause after losses
    //! - `stop_loss_pct`: Pre-defined, not emotion-driven
    //! - `consecutive_losses`: Hard limit triggers halt
    //!
    //! ## Healthy Alternative
    //! Accept losses as part of the process.
    //! Define exits before entry, not during drawdown.

    //! # Delusion (Moha)
    //!
    //! ## Manifestation
    //! - Believing lagging indicators predict futures
    //! - Acting on stale or insufficient data
    //! - Over-fitting to recent performance
    //!
    //! ## Guards
    //! - `data_stale_ms`: Refuse to act on old data
    //! - Minimum candle count before signals valid
    //! - Mean-reversion over momentum-chasing
    //!
    //! ## Healthy Alternative
    //! Acknowledge uncertainty as fundamental.
    //! Use multiple confirmation, not single indicators.
    //! Provide liquidity rather than chase trends.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::state::EngineState;
    use crate::engine::reducer::ReducerConfig;

    #[test]
    fn test_greed_over_extension() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            max_position_pct: 0.05,
            ..Default::default()
        };

        // Add a position that exceeds limit
        state.portfolio.cash = 9000.0;
        state.portfolio.equity = 10000.0;
        state.portfolio.positions.insert(
            "BTCUSDT".to_string(),
            crate::engine::state::Position { qty: 0.1, entry_price: 50000.0 }
        );
        state.symbols.insert(
            "BTCUSDT".to_string(),
            {
                let mut sym = crate::engine::state::SymbolState::new();
                sym.last_price = 50000.0;
                sym.candle_count = 20;
                sym
            }
        );

        let result = check_three_poisons(&state, "BTCUSDT", &cfg);
        assert!(matches!(result, Some(EthicsViolation::GreedOverExtension { .. })));
    }

    #[test]
    fn test_aversion_revenge_trading() {
        let mut state = EngineState::new();
        state.now = 100_000;
        state.risk.last_loss_ts = 95_000; // Loss 5 seconds ago

        let cfg = ReducerConfig {
            cooldown_ms: 10_000, // 10 second cooldown
            ..Default::default()
        };

        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = crate::engine::state::SymbolState::new();
            sym.candle_count = 20;
            sym.last_ts = 100_000;
            sym
        });

        let result = check_three_poisons(&state, "BTCUSDT", &cfg);
        assert!(matches!(result, Some(EthicsViolation::AversionRevenge { .. })));
    }

    #[test]
    fn test_delusion_insufficient_data() {
        let mut state = EngineState::new();
        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = crate::engine::state::SymbolState::new();
            sym.candle_count = 3; // Only 3 candles
            sym.last_ts = state.now;
            sym
        });

        let cfg = ReducerConfig::default();
        let result = check_three_poisons(&state, "BTCUSDT", &cfg);
        assert!(matches!(result, Some(EthicsViolation::DelusionInsufficient { .. })));
    }

    #[test]
    fn test_aligned_state_passes() {
        let mut state = EngineState::new();
        state.now = 100_000;

        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = crate::engine::state::SymbolState::new();
            sym.candle_count = 20;
            sym.last_ts = 100_000;
            sym.last_price = 50000.0;
            sym
        });

        let cfg = ReducerConfig::default();
        let result = check_three_poisons(&state, "BTCUSDT", &cfg);
        assert!(result.is_none(), "Aligned state should pass all guards");
    }
}
