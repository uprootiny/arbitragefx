//! Trade filters - conditions that must be met before entering a trade.
//!
//! Filters are composable: combine multiple filters with AND/OR logic.

use crate::strategy::{IndicatorSnapshot, MarketAux, StrategyState};

/// Filter result with reason
#[derive(Debug, Clone)]
pub struct FilterResult {
    pub passed: bool,
    pub reason: &'static str,
}

impl FilterResult {
    pub fn pass() -> Self {
        Self { passed: true, reason: "passed" }
    }

    pub fn fail(reason: &'static str) -> Self {
        Self { passed: false, reason }
    }
}

// =============================================================================
// Volatility Filters
// =============================================================================

/// Block trades when volatility is too high or too low
pub fn volatility_filter(ind: &IndicatorSnapshot, min_vol: f64, max_vol: f64) -> FilterResult {
    let vol_ratio = if ind.vol_mean > 0.0 { ind.vol / ind.vol_mean } else { 1.0 };

    if vol_ratio < min_vol {
        FilterResult::fail("vol_too_low")
    } else if vol_ratio > max_vol {
        FilterResult::fail("vol_too_high")
    } else {
        FilterResult::pass()
    }
}

/// Block trades during extreme volatility spikes
pub fn vol_spike_filter(ind: &IndicatorSnapshot, max_z: f64) -> FilterResult {
    if ind.z_vol > max_z {
        FilterResult::fail("vol_spike")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Trend Filters
// =============================================================================

/// Only allow trades in direction of trend
pub fn trend_alignment_filter(ind: &IndicatorSnapshot, direction: f64) -> FilterResult {
    if ind.ema_slow <= 0.0 {
        return FilterResult::fail("no_trend_data");
    }

    let trend_dir = if ind.ema_fast > ind.ema_slow { 1.0 } else { -1.0 };
    if trend_dir * direction >= 0.0 {
        FilterResult::pass()
    } else {
        FilterResult::fail("against_trend")
    }
}

/// Require minimum trend strength
pub fn trend_strength_filter(ind: &IndicatorSnapshot, min_strength: f64) -> FilterResult {
    if ind.ema_slow <= 0.0 {
        return FilterResult::fail("no_trend_data");
    }

    let strength = (ind.ema_fast - ind.ema_slow).abs() / ind.ema_slow;
    if strength >= min_strength {
        FilterResult::pass()
    } else {
        FilterResult::fail("weak_trend")
    }
}

/// Block trades during trend transitions (EMA crossover zone)
pub fn trend_transition_filter(ind: &IndicatorSnapshot, deadzone_pct: f64) -> FilterResult {
    if ind.ema_slow <= 0.0 {
        return FilterResult::fail("no_trend_data");
    }

    let divergence = (ind.ema_fast - ind.ema_slow).abs() / ind.ema_slow;
    if divergence < deadzone_pct {
        FilterResult::fail("trend_transition")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Momentum Filters
// =============================================================================

/// Require minimum momentum for entries
pub fn momentum_filter(ind: &IndicatorSnapshot, min_z: f64) -> FilterResult {
    if ind.z_momentum.abs() >= min_z {
        FilterResult::pass()
    } else {
        FilterResult::fail("weak_momentum")
    }
}

/// Block trades when momentum is exhausted (stretched too far)
pub fn momentum_exhaustion_filter(ind: &IndicatorSnapshot, max_stretch: f64) -> FilterResult {
    if ind.z_stretch.abs() > max_stretch {
        FilterResult::fail("momentum_exhausted")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Position/Risk Filters
// =============================================================================

/// Limit position size as percentage of equity
pub fn position_limit_filter(state: &StrategyState, price: f64, max_pct: f64) -> FilterResult {
    let position_value = state.portfolio.position.abs() * price;
    let equity = state.portfolio.equity;
    if equity <= 0.0 {
        return FilterResult::fail("no_equity");
    }

    let current_pct = position_value / equity;
    if current_pct >= max_pct {
        FilterResult::fail("position_limit")
    } else {
        FilterResult::pass()
    }
}

/// Cooldown after recent loss
pub fn loss_cooldown_filter(state: &StrategyState, now_ts: u64, cooldown_secs: u64) -> FilterResult {
    if state.last_loss_ts == 0 {
        return FilterResult::pass();
    }

    let elapsed = now_ts.saturating_sub(state.last_loss_ts);
    if elapsed < cooldown_secs {
        FilterResult::fail("loss_cooldown")
    } else {
        FilterResult::pass()
    }
}

/// Maximum trades per day
pub fn daily_trade_limit_filter(state: &StrategyState, now_ts: u64, max_trades: u32) -> FilterResult {
    let current_day = now_ts / 86400;
    let trades_today = if state.trade_day == current_day {
        state.trades_today
    } else {
        0
    };

    if trades_today >= max_trades {
        FilterResult::fail("daily_limit")
    } else {
        FilterResult::pass()
    }
}

/// Block trades during drawdown
pub fn drawdown_filter(state: &StrategyState, max_dd: f64) -> FilterResult {
    let dd = state.metrics.max_drawdown;
    if dd < -max_dd {
        FilterResult::fail("drawdown_limit")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Market Condition Filters
// =============================================================================

/// Require fresh aux data
pub fn data_freshness_filter(aux: &MarketAux, now_ts: u64, max_age_secs: u64) -> FilterResult {
    if aux.fetch_ts == 0 {
        return FilterResult::fail("no_aux_data");
    }

    let age = now_ts.saturating_sub(aux.fetch_ts);
    if age > max_age_secs {
        FilterResult::fail("stale_data")
    } else {
        FilterResult::pass()
    }
}

/// Block during extreme funding rates
pub fn funding_extreme_filter(aux: &MarketAux, max_rate: f64) -> FilterResult {
    if !aux.has_funding {
        return FilterResult::pass(); // No data, allow trade
    }

    if aux.funding_rate.abs() > max_rate {
        FilterResult::fail("extreme_funding")
    } else {
        FilterResult::pass()
    }
}

/// Block during high liquidation activity
pub fn liquidation_filter(aux: &MarketAux, max_score: f64) -> FilterResult {
    if !aux.has_liquidations {
        return FilterResult::pass();
    }

    if aux.liquidation_score > max_score {
        FilterResult::fail("high_liquidations")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Time Filters
// =============================================================================

/// Filter by hour of day (UTC)
pub fn time_of_day_filter(ts: u64, allowed_hours: &[u8]) -> FilterResult {
    let hour = ((ts % 86400) / 3600) as u8;
    if allowed_hours.contains(&hour) {
        FilterResult::pass()
    } else {
        FilterResult::fail("wrong_hour")
    }
}

/// Filter by day of week (0 = Sunday)
pub fn day_of_week_filter(ts: u64, allowed_days: &[u8]) -> FilterResult {
    // Unix timestamp 0 = Thursday, so adjust
    let day = ((ts / 86400 + 4) % 7) as u8;
    if allowed_days.contains(&day) {
        FilterResult::pass()
    } else {
        FilterResult::fail("wrong_day")
    }
}

/// Block around known events (e.g., funding settlement every 8h)
pub fn funding_settlement_filter(ts: u64, buffer_secs: u64) -> FilterResult {
    // Funding settles every 8 hours at 00:00, 08:00, 16:00 UTC
    let hour = (ts % 86400) / 3600;
    let minute = (ts % 3600) / 60;
    let secs_to_settlement = if hour % 8 == 7 && minute >= 55 {
        (60 - minute) * 60
    } else if hour % 8 == 0 && minute < 5 {
        0 // Just after settlement
    } else {
        u64::MAX
    };

    if secs_to_settlement < buffer_secs {
        FilterResult::fail("funding_settlement")
    } else {
        FilterResult::pass()
    }
}

// =============================================================================
// Composite Filters
// =============================================================================

/// AND combinator: all filters must pass
pub fn all_filters(results: &[FilterResult]) -> FilterResult {
    for r in results {
        if !r.passed {
            return r.clone();
        }
    }
    FilterResult::pass()
}

/// OR combinator: at least one filter must pass
pub fn any_filter(results: &[FilterResult]) -> FilterResult {
    for r in results {
        if r.passed {
            return FilterResult::pass();
        }
    }
    FilterResult::fail("all_filters_failed")
}

/// Count passing filters
pub fn count_passing(results: &[FilterResult]) -> usize {
    results.iter().filter(|r| r.passed).count()
}

// =============================================================================
// Filter Presets
// =============================================================================

/// Standard filter set for trend-following strategies
pub fn trend_following_filters(
    ind: &IndicatorSnapshot,
    state: &StrategyState,
    direction: f64,
    price: f64,
) -> FilterResult {
    all_filters(&[
        volatility_filter(ind, 0.3, 3.0),
        trend_alignment_filter(ind, direction),
        position_limit_filter(state, price, 0.1),
        drawdown_filter(state, 0.1),
    ])
}

/// Standard filter set for mean reversion strategies
pub fn mean_reversion_filters(
    ind: &IndicatorSnapshot,
    state: &StrategyState,
    price: f64,
) -> FilterResult {
    all_filters(&[
        volatility_filter(ind, 0.5, 2.0), // Tighter vol range for reversion
        trend_transition_filter(ind, 0.001), // Avoid during crossovers
        position_limit_filter(state, price, 0.05), // Smaller positions
        drawdown_filter(state, 0.05),
    ])
}

/// Aggressive scalping filters
pub fn scalping_filters(
    ind: &IndicatorSnapshot,
    state: &StrategyState,
    price: f64,
    now_ts: u64,
) -> FilterResult {
    all_filters(&[
        volatility_filter(ind, 0.8, 1.5), // Need volatility but not extreme
        vol_spike_filter(ind, 2.0),
        position_limit_filter(state, price, 0.02),
        loss_cooldown_filter(state, now_ts, 300), // 5 min cooldown
        daily_trade_limit_filter(state, now_ts, 50),
    ])
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{PortfolioState, MetricsState};

    fn make_ind(z_mom: f64, z_vol: f64, ema_fast: f64, ema_slow: f64) -> IndicatorSnapshot {
        IndicatorSnapshot {
            ema_fast,
            ema_slow,
            z_momentum: z_mom,
            z_vol,
            vol: 1.0,
            vol_mean: 1.0,
            ..Default::default()
        }
    }

    fn make_state() -> StrategyState {
        StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        }
    }

    #[test]
    fn test_trend_alignment_bullish() {
        let ind = make_ind(0.0, 0.0, 101.0, 100.0); // Uptrend
        assert!(trend_alignment_filter(&ind, 1.0).passed); // Long ok
        assert!(!trend_alignment_filter(&ind, -1.0).passed); // Short blocked
    }

    #[test]
    fn test_volatility_filter() {
        let mut ind = make_ind(0.0, 0.0, 100.0, 100.0);
        ind.vol = 0.5;
        ind.vol_mean = 1.0;
        assert!(volatility_filter(&ind, 0.3, 2.0).passed); // Normal vol
        assert!(!volatility_filter(&ind, 0.6, 2.0).passed); // Too low
    }

    #[test]
    fn test_position_limit() {
        let mut state = make_state();
        state.portfolio.position = 0.05; // 5% at $1000 price
        state.portfolio.equity = 1000.0;

        assert!(position_limit_filter(&state, 1000.0, 0.1).passed); // Under limit
        assert!(!position_limit_filter(&state, 1000.0, 0.04).passed); // Over limit
    }

    #[test]
    fn test_all_filters() {
        let results = vec![FilterResult::pass(), FilterResult::pass()];
        assert!(all_filters(&results).passed);

        let results2 = vec![FilterResult::pass(), FilterResult::fail("test")];
        assert!(!all_filters(&results2).passed);
    }
}
