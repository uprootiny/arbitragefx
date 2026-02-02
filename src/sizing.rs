//! Position sizing algorithms.
//!
//! All sizing functions return a quantity (in base asset units).

use crate::strategy::{IndicatorSnapshot, StrategyState};

// =============================================================================
// Fixed Sizing
// =============================================================================

/// Fixed quantity per trade
pub fn fixed_size(qty: f64) -> f64 {
    qty
}

/// Fixed percentage of equity
pub fn fixed_equity_pct(equity: f64, pct: f64, price: f64) -> f64 {
    let value = equity * pct;
    value / price
}

/// Fixed notional value per trade
pub fn fixed_notional(notional: f64, price: f64) -> f64 {
    notional / price
}

// =============================================================================
// Risk-Based Sizing
// =============================================================================

/// Size based on risk per trade (stop loss distance)
///
/// risk_per_trade: e.g., 0.01 = risk 1% of equity per trade
/// stop_distance: distance to stop loss as fraction of price (e.g., 0.02 = 2%)
pub fn risk_based_size(equity: f64, risk_per_trade: f64, stop_distance: f64, price: f64) -> f64 {
    if stop_distance <= 0.0 || price <= 0.0 {
        return 0.0;
    }
    let risk_amount = equity * risk_per_trade;
    let loss_per_unit = price * stop_distance;
    risk_amount / loss_per_unit
}

/// Size adjusted for volatility (higher vol = smaller size)
pub fn volatility_adjusted_size(
    base_size: f64,
    target_vol: f64,
    current_vol: f64,
) -> f64 {
    if current_vol <= 0.0 {
        return base_size;
    }
    base_size * (target_vol / current_vol).min(2.0).max(0.25)
}

/// ATR-based sizing: size inversely proportional to ATR
pub fn atr_sized(equity: f64, risk_pct: f64, atr: f64, price: f64, atr_multiple: f64) -> f64 {
    if atr <= 0.0 || price <= 0.0 {
        return 0.0;
    }
    let risk_amount = equity * risk_pct;
    let stop_distance = atr * atr_multiple;
    risk_amount / stop_distance
}

// =============================================================================
// Kelly Criterion
// =============================================================================

/// Full Kelly sizing
///
/// win_rate: probability of winning (0.0 - 1.0)
/// avg_win: average win amount
/// avg_loss: average loss amount (positive number)
pub fn kelly_size(win_rate: f64, avg_win: f64, avg_loss: f64) -> f64 {
    if avg_loss <= 0.0 || win_rate <= 0.0 || win_rate >= 1.0 {
        return 0.0;
    }

    let win_loss_ratio = avg_win / avg_loss;
    let kelly = win_rate - (1.0 - win_rate) / win_loss_ratio;

    kelly.max(0.0)
}

/// Fractional Kelly (safer, typically 0.25-0.5 of full Kelly)
pub fn fractional_kelly(win_rate: f64, avg_win: f64, avg_loss: f64, fraction: f64) -> f64 {
    kelly_size(win_rate, avg_win, avg_loss) * fraction
}

/// Kelly position size in base asset units
pub fn kelly_position(
    equity: f64,
    win_rate: f64,
    avg_win: f64,
    avg_loss: f64,
    fraction: f64,
    price: f64,
) -> f64 {
    let kelly_pct = fractional_kelly(win_rate, avg_win, avg_loss, fraction);
    let value = equity * kelly_pct;
    value / price
}

/// Calculate Kelly from metrics state
pub fn kelly_from_metrics(state: &StrategyState, fraction: f64, price: f64) -> f64 {
    let total = state.metrics.wins + state.metrics.losses;
    if total < 10 {
        return 0.0; // Not enough data
    }

    let win_rate = state.metrics.wins as f64 / total as f64;
    let avg_win = if state.metrics.wins > 0 {
        state.metrics.total_win_amount / state.metrics.wins as f64
    } else {
        0.0
    };
    let avg_loss = if state.metrics.losses > 0 {
        state.metrics.total_loss_amount / state.metrics.losses as f64
    } else {
        0.0
    };

    kelly_position(state.portfolio.equity, win_rate, avg_win, avg_loss, fraction, price)
}

// =============================================================================
// Signal-Based Sizing
// =============================================================================

/// Size proportional to signal strength
pub fn signal_scaled_size(base_size: f64, signal_strength: f64, max_multiple: f64) -> f64 {
    let multiplier = 1.0 + (signal_strength.clamp(0.0, 1.0) * (max_multiple - 1.0));
    base_size * multiplier
}

/// Size based on confidence level
pub fn confidence_scaled_size(base_size: f64, confidence: f64) -> f64 {
    base_size * confidence.clamp(0.0, 1.0)
}

/// Size inversely proportional to number of open positions (for diversification)
pub fn diversified_size(base_size: f64, num_positions: usize, max_positions: usize) -> f64 {
    if num_positions >= max_positions {
        return 0.0;
    }
    let slots_remaining = max_positions - num_positions;
    base_size * (slots_remaining as f64 / max_positions as f64)
}

// =============================================================================
// Pyramiding
// =============================================================================

/// Calculate size for adding to winning position
pub fn pyramid_size(
    initial_size: f64,
    current_position: f64,
    max_position: f64,
    scale_factor: f64,
) -> f64 {
    let position_pct = current_position.abs() / max_position;
    if position_pct >= 1.0 {
        return 0.0;
    }

    // Each add is smaller than the previous
    let add_size = initial_size * scale_factor.powi((current_position.abs() / initial_size) as i32);
    let remaining_capacity = max_position - current_position.abs();
    add_size.min(remaining_capacity)
}

/// Calculate if we should add to position (price moved favorably)
pub fn should_pyramid(
    entry_price: f64,
    current_price: f64,
    is_long: bool,
    min_profit_pct: f64,
) -> bool {
    let profit_pct = if is_long {
        (current_price - entry_price) / entry_price
    } else {
        (entry_price - current_price) / entry_price
    };
    profit_pct >= min_profit_pct
}

// =============================================================================
// Position Limits
// =============================================================================

/// Apply maximum position limit
pub fn apply_max_position(requested: f64, current: f64, max: f64) -> f64 {
    let would_be = current + requested;
    if would_be.abs() > max {
        let available = max - current.abs();
        if requested > 0.0 {
            available.max(0.0)
        } else {
            -available.max(0.0)
        }
    } else {
        requested
    }
}

/// Apply maximum notional value limit
pub fn apply_max_notional(requested: f64, current: f64, max_notional: f64, price: f64) -> f64 {
    let current_notional = current.abs() * price;
    let requested_notional = requested.abs() * price;
    let would_be = current_notional + requested_notional;

    if would_be > max_notional {
        let available = (max_notional - current_notional) / price;
        if requested > 0.0 {
            available.max(0.0)
        } else {
            -available.max(0.0)
        }
    } else {
        requested
    }
}

/// Apply minimum trade size (exchange minimums)
pub fn apply_min_size(requested: f64, min_size: f64) -> f64 {
    if requested.abs() < min_size {
        0.0
    } else {
        requested
    }
}

/// Round to exchange lot size
pub fn round_to_lot(qty: f64, lot_size: f64) -> f64 {
    if lot_size <= 0.0 {
        return qty;
    }
    (qty / lot_size).floor() * lot_size
}

// =============================================================================
// Composite Sizing
// =============================================================================

/// Standard position sizer combining multiple rules
pub struct PositionSizer {
    pub base_size: f64,
    pub max_position: f64,
    pub max_equity_pct: f64,
    pub kelly_fraction: f64,
    pub use_volatility_scaling: bool,
    pub target_volatility: f64,
    pub min_size: f64,
    pub lot_size: f64,
}

impl Default for PositionSizer {
    fn default() -> Self {
        Self {
            base_size: 0.001,
            max_position: 0.01,
            max_equity_pct: 0.05,
            kelly_fraction: 0.25,
            use_volatility_scaling: true,
            target_volatility: 0.02,
            min_size: 0.0001,
            lot_size: 0.0001,
        }
    }
}

impl PositionSizer {
    /// Calculate final position size applying all rules
    pub fn calculate(
        &self,
        state: &StrategyState,
        ind: &IndicatorSnapshot,
        price: f64,
        signal_strength: f64,
    ) -> f64 {
        let mut size = self.base_size;

        // Scale by signal strength
        size = signal_scaled_size(size, signal_strength, 2.0);

        // Scale by volatility if enabled
        if self.use_volatility_scaling && ind.vol > 0.0 {
            size = volatility_adjusted_size(size, self.target_volatility, ind.vol);
        }

        // Apply Kelly if we have enough data
        let total_trades = state.metrics.wins + state.metrics.losses;
        if total_trades >= 20 {
            let kelly = kelly_from_metrics(state, self.kelly_fraction, price);
            if kelly > 0.0 {
                size = size.min(kelly);
            }
        }

        // Apply position limits
        size = apply_max_position(size, state.portfolio.position, self.max_position);

        // Apply equity limit
        let max_from_equity = state.portfolio.equity * self.max_equity_pct / price;
        size = size.min(max_from_equity);

        // Apply minimum and lot size
        size = apply_min_size(size, self.min_size);
        size = round_to_lot(size, self.lot_size);

        size
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{PortfolioState, MetricsState};

    #[test]
    fn test_fixed_equity_pct() {
        let qty = fixed_equity_pct(10000.0, 0.01, 100.0);
        assert!((qty - 1.0).abs() < 0.001); // 1% of 10000 at $100 = 1 unit
    }

    #[test]
    fn test_risk_based_size() {
        // Risk 1% of $10000 with 2% stop = risk $100, stop at $2 per unit = 50 units
        let qty = risk_based_size(10000.0, 0.01, 0.02, 100.0);
        assert!((qty - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_kelly_size() {
        // 60% win rate, avg win $2, avg loss $1
        // Kelly = 0.6 - 0.4/2 = 0.6 - 0.2 = 0.4
        let k = kelly_size(0.6, 2.0, 1.0);
        assert!((k - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_kelly_negative_edge() {
        // 40% win rate, avg win $1, avg loss $1 → negative edge
        let k = kelly_size(0.4, 1.0, 1.0);
        assert!(k < 0.001); // Should return ~0
    }

    #[test]
    fn test_volatility_adjusted() {
        let size = volatility_adjusted_size(1.0, 0.02, 0.04);
        assert!((size - 0.5).abs() < 0.001); // 2x vol = half size
    }

    #[test]
    fn test_apply_max_position() {
        let result = apply_max_position(0.5, 0.8, 1.0);
        assert!((result - 0.2).abs() < 0.001, "Expected 0.2, got {}", result); // Can only add 0.2
        assert_eq!(apply_max_position(0.5, 0.3, 1.0), 0.5); // Full size ok
    }

    #[test]
    fn test_round_to_lot() {
        assert_eq!(round_to_lot(1.234, 0.01), 1.23);
        assert_eq!(round_to_lot(1.239, 0.01), 1.23);
    }
}
