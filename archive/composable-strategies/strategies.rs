//! Composable strategy implementations using signal building blocks.
//!
//! Each strategy combines:
//! - Entry signals (when to open)
//! - Exit rules (when to close)
//! - Filters (when to abstain)
//! - Position sizing (how much)

use crate::signals::{
    climax_signal, funding_carry_signal, funding_extreme_signal, liquidation_cascade_signal,
    mean_reversion_signal, momentum_signal, multi_factor, trend_aligned_momentum,
    trend_aligned_reversion, trend_strength, volatility_regime,
    Signal, SignalWeights, VolRegime,
};
use crate::strategy::{Action, MarketView, StrategyState, Strategy};

// =============================================================================
// Strategy Configuration
// =============================================================================

/// Common configuration for all strategies
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Minimum signal strength to enter
    pub entry_threshold: f64,
    /// Signal strength below which to exit
    pub exit_threshold: f64,
    /// Stop loss percentage
    pub stop_loss: f64,
    /// Take profit percentage
    pub take_profit: f64,
    /// Maximum bars to hold position
    pub max_hold_bars: u64,
    /// Cooldown bars after loss
    pub loss_cooldown_bars: u64,
    /// Base position size
    pub base_size: f64,
    /// Candle interval in seconds
    pub candle_secs: u64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            entry_threshold: 0.3,
            exit_threshold: 0.15,
            stop_loss: 0.02,
            take_profit: 0.03,
            max_hold_bars: 48,
            loss_cooldown_bars: 4,
            base_size: 0.001,
            candle_secs: 300, // 5m default
        }
    }
}

// =============================================================================
// Exit Rules
// =============================================================================

/// Check if stop loss hit
fn check_stop_loss(state: &StrategyState, price: f64, stop_pct: f64) -> bool {
    if state.portfolio.position == 0.0 {
        return false;
    }
    let entry = state.portfolio.entry_price;
    if entry <= 0.0 {
        return false;
    }
    let move_pct = if state.portfolio.position > 0.0 {
        (price - entry) / entry
    } else {
        (entry - price) / entry
    };
    move_pct <= -stop_pct
}

/// Check if take profit hit
fn check_take_profit(state: &StrategyState, price: f64, profit_pct: f64) -> bool {
    if state.portfolio.position == 0.0 {
        return false;
    }
    let entry = state.portfolio.entry_price;
    if entry <= 0.0 {
        return false;
    }
    let move_pct = if state.portfolio.position > 0.0 {
        (price - entry) / entry
    } else {
        (entry - price) / entry
    };
    move_pct >= profit_pct
}

/// Check if time stop hit
fn check_time_stop(state: &StrategyState, now_ts: u64, max_bars: u64, candle_secs: u64) -> bool {
    if state.portfolio.position == 0.0 || state.last_trade_ts == 0 {
        return false;
    }
    let bars_held = (now_ts.saturating_sub(state.last_trade_ts)) / candle_secs;
    bars_held >= max_bars
}

/// Check if in cooldown after loss
fn check_cooldown(state: &StrategyState, now_ts: u64, cooldown_bars: u64, candle_secs: u64) -> bool {
    if state.last_loss_ts == 0 {
        return false;
    }
    let bars_since_loss = (now_ts.saturating_sub(state.last_loss_ts)) / candle_secs;
    bars_since_loss < cooldown_bars
}

// =============================================================================
// Pure Momentum Strategy
// =============================================================================

/// Pure momentum: follow price momentum with trend confirmation
pub struct MomentumStrategy {
    pub config: StrategyConfig,
    pub momentum_threshold: f64,
    pub require_trend_align: bool,
}

impl Default for MomentumStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig::default(),
            momentum_threshold: 1.0,
            require_trend_align: true,
        }
    }
}

impl Strategy for MomentumStrategy {
    fn id(&self) -> &'static str {
        "momentum"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        // Cooldown check
        if check_cooldown(state, now, self.config.loss_cooldown_bars, self.config.candle_secs) {
            return Action::Hold;
        }

        // Exit checks for existing position
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }
        }

        // Get signal
        let signal = if self.require_trend_align {
            trend_aligned_momentum(&market.indicators, self.momentum_threshold)
        } else {
            momentum_signal(&market.indicators, self.momentum_threshold)
        };

        // Already in position - check for exit signal
        if state.portfolio.position != 0.0 {
            let holding_long = state.portfolio.position > 0.0;
            let signal_reversed = (holding_long && signal.is_bearish())
                || (!holding_long && signal.is_bullish());
            if signal_reversed || signal.strength < self.config.exit_threshold {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Entry logic
        if signal.strength > self.config.entry_threshold {
            if signal.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if signal.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Mean Reversion Strategy
// =============================================================================

/// Mean reversion: fade extreme stretches, optionally trend-aligned
pub struct MeanReversionStrategy {
    pub config: StrategyConfig,
    pub stretch_threshold: f64,
    pub require_trend_align: bool,
}

impl Default for MeanReversionStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.015,
                take_profit: 0.01,
                max_hold_bars: 12,
                ..Default::default()
            },
            stretch_threshold: 2.0,
            require_trend_align: true,
        }
    }
}

impl Strategy for MeanReversionStrategy {
    fn id(&self) -> &'static str {
        "mean_reversion"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        if check_cooldown(state, now, self.config.loss_cooldown_bars, self.config.candle_secs) {
            return Action::Hold;
        }

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }
            // Exit when stretch normalizes
            if market.indicators.z_stretch.abs() < 0.5 {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Only trade in low/normal volatility (high vol mean reversion is risky)
        let vol_regime = volatility_regime(&market.indicators);
        if matches!(vol_regime, VolRegime::High | VolRegime::Extreme) {
            return Action::Hold;
        }

        // Get signal
        let signal = if self.require_trend_align {
            trend_aligned_reversion(&market.indicators, self.stretch_threshold)
        } else {
            mean_reversion_signal(&market.indicators, self.stretch_threshold)
        };

        // Entry
        if signal.strength > self.config.entry_threshold {
            if signal.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if signal.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Funding Carry Strategy
// =============================================================================

/// Funding carry: collect funding payments by being opposite the crowd
pub struct FundingCarryStrategy {
    pub config: StrategyConfig,
    pub funding_threshold: f64,
    pub spread_min: f64,
}

impl Default for FundingCarryStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.01,
                take_profit: 0.005, // Smaller TP, rely on funding
                max_hold_bars: 24,  // Hold for ~8h to collect funding
                ..Default::default()
            },
            funding_threshold: 0.0005, // 0.05% funding
            spread_min: 0.0002,        // Net carry must exceed 0.02%
        }
    }
}

impl Strategy for FundingCarryStrategy {
    fn id(&self) -> &'static str {
        "funding_carry"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let _now = market.last.ts;
        let price = market.last.c;

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            // For carry, we want to hold longer, so wider take profit
            if check_take_profit(state, price, self.config.take_profit * 2.0) {
                return Action::Close;
            }
            // Check if funding flipped against us
            let signal = funding_carry_signal(&market.aux, self.funding_threshold, self.spread_min);
            let holding_long = state.portfolio.position > 0.0;
            if (holding_long && signal.is_bearish()) || (!holding_long && signal.is_bullish()) {
                // Funding now favors opposite direction
                return Action::Close;
            }
            return Action::Hold;
        }

        // Entry signal
        let signal = funding_carry_signal(&market.aux, self.funding_threshold, self.spread_min);

        // Also check funding isn't at extreme (reversal risk)
        let extreme = funding_extreme_signal(&market.aux, 0.003);
        if !extreme.is_neutral() {
            // Funding is extreme - don't enter new carry position
            return Action::Hold;
        }

        if signal.strength > self.config.entry_threshold {
            if signal.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if signal.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Volatility Breakout Strategy
// =============================================================================

/// Volatility breakout: trade breakouts from low vol with volume confirmation
pub struct VolatilityBreakoutStrategy {
    pub config: StrategyConfig,
    pub vol_expansion_threshold: f64,
    pub volume_confirmation_threshold: f64,
}

impl Default for VolatilityBreakoutStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.015,
                take_profit: 0.025,
                max_hold_bars: 24,
                ..Default::default()
            },
            vol_expansion_threshold: 1.5,
            volume_confirmation_threshold: 1.5,
        }
    }
}

impl Strategy for VolatilityBreakoutStrategy {
    fn id(&self) -> &'static str {
        "vol_breakout"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        if check_cooldown(state, now, self.config.loss_cooldown_bars, self.config.candle_secs) {
            return Action::Hold;
        }

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Need vol expansion + volume confirmation
        if market.indicators.z_vol < self.vol_expansion_threshold {
            return Action::Hold;
        }
        if market.indicators.z_volume_spike < self.volume_confirmation_threshold {
            return Action::Hold;
        }

        // Direction from momentum
        let mom = momentum_signal(&market.indicators, 0.5);
        if mom.strength > self.config.entry_threshold {
            if mom.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if mom.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Event-Driven Strategy
// =============================================================================

/// Event-driven: trade liquidation cascades and stablecoin depegs
pub struct EventDrivenStrategy {
    pub config: StrategyConfig,
    pub liquidation_threshold: f64,
    pub depeg_threshold: f64,
}

impl Default for EventDrivenStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.02,
                take_profit: 0.03,
                max_hold_bars: 8, // Events are short-lived
                ..Default::default()
            },
            liquidation_threshold: 1.5,
            depeg_threshold: 0.005,
        }
    }
}

impl Strategy for EventDrivenStrategy {
    fn id(&self) -> &'static str {
        "event_driven"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Liquidation cascade signal
        let liq = liquidation_cascade_signal(&market.aux, &market.indicators, self.liquidation_threshold);
        if liq.strength > self.config.entry_threshold {
            if liq.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if liq.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        // Climax reversal signal (extreme vol + extreme price)
        let climax = climax_signal(&market.indicators);
        if climax.strength > 0.5 {
            if climax.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if climax.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Multi-Factor Strategy
// =============================================================================

/// Multi-factor: combine multiple signals with configurable weights
pub struct MultiFactorStrategy {
    pub config: StrategyConfig,
    pub weights: SignalWeights,
}

impl Default for MultiFactorStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig::default(),
            weights: SignalWeights::default(),
        }
    }
}

impl MultiFactorStrategy {
    pub fn trend_following() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.025,
                take_profit: 0.04,
                max_hold_bars: 72,
                ..Default::default()
            },
            weights: SignalWeights::trend_following(),
        }
    }

    pub fn mean_reversion_focused() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.015,
                take_profit: 0.012,
                max_hold_bars: 16,
                ..Default::default()
            },
            weights: SignalWeights::mean_reversion(),
        }
    }

    pub fn carry_focused() -> Self {
        Self {
            config: StrategyConfig {
                stop_loss: 0.01,
                take_profit: 0.008,
                max_hold_bars: 48,
                ..Default::default()
            },
            weights: SignalWeights::carry(),
        }
    }
}

impl Strategy for MultiFactorStrategy {
    fn id(&self) -> &'static str {
        "multi_factor"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        if check_cooldown(state, now, self.config.loss_cooldown_bars, self.config.candle_secs) {
            return Action::Hold;
        }

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }

            // Exit on signal reversal
            let signal = multi_factor(&market.indicators, &market.aux, &self.weights);
            let holding_long = state.portfolio.position > 0.0;
            if (holding_long && signal.is_bearish() && signal.strength > 0.3)
                || (!holding_long && signal.is_bullish() && signal.strength > 0.3)
            {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Entry
        let signal = multi_factor(&market.indicators, &market.aux, &self.weights);
        if signal.strength > self.config.entry_threshold {
            if signal.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if signal.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Adaptive Strategy
// =============================================================================

/// Adaptive: switch between momentum and mean reversion based on regime
pub struct AdaptiveStrategy {
    pub config: StrategyConfig,
    pub momentum_threshold: f64,
    pub reversion_threshold: f64,
}

impl Default for AdaptiveStrategy {
    fn default() -> Self {
        Self {
            config: StrategyConfig::default(),
            momentum_threshold: 1.0,
            reversion_threshold: 2.0,
        }
    }
}

impl Strategy for AdaptiveStrategy {
    fn id(&self) -> &'static str {
        "adaptive"
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action {
        let now = market.last.ts;
        let price = market.last.c;

        if check_cooldown(state, now, self.config.loss_cooldown_bars, self.config.candle_secs) {
            return Action::Hold;
        }

        // Exit checks
        if state.portfolio.position != 0.0 {
            if check_stop_loss(state, price, self.config.stop_loss) {
                return Action::Close;
            }
            if check_take_profit(state, price, self.config.take_profit) {
                return Action::Close;
            }
            if check_time_stop(state, now, self.config.max_hold_bars, self.config.candle_secs) {
                return Action::Close;
            }
            return Action::Hold;
        }

        // Determine regime
        let vol_regime = volatility_regime(&market.indicators);
        let ts = trend_strength(&market.indicators);
        let strong_trend = ts > 0.015; // 1.5% EMA divergence

        // Regime-based strategy selection
        let signal = match vol_regime {
            VolRegime::Low => {
                // Low vol: momentum works well
                trend_aligned_momentum(&market.indicators, self.momentum_threshold)
            }
            VolRegime::Normal if strong_trend => {
                // Normal vol + trend: follow momentum
                trend_aligned_momentum(&market.indicators, self.momentum_threshold)
            }
            VolRegime::Normal => {
                // Normal vol + no trend: mean reversion
                mean_reversion_signal(&market.indicators, self.reversion_threshold)
            }
            VolRegime::High if strong_trend => {
                // High vol + trend: momentum but more selective
                let mom = trend_aligned_momentum(&market.indicators, self.momentum_threshold * 1.5);
                Signal { strength: mom.strength * 0.8, ..mom }
            }
            VolRegime::High => {
                // High vol + no trend: stand aside or careful reversion
                trend_aligned_reversion(&market.indicators, self.reversion_threshold * 1.5)
            }
            VolRegime::Extreme => {
                // Extreme vol: stand aside
                Signal::neutral()
            }
        };

        // Entry
        if signal.strength > self.config.entry_threshold {
            if signal.is_bullish() {
                return Action::Buy { qty: self.config.base_size };
            } else if signal.is_bearish() {
                return Action::Sell { qty: self.config.base_size };
            }
        }

        Action::Hold
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{Candle, IndicatorSnapshot, MarketAux, PortfolioState, MetricsState};

    fn make_market(z_mom: f64, z_stretch: f64, z_vol: f64) -> MarketView<'static> {
        // Using a leaked static string for simplicity in tests
        // Default: slight uptrend (1% EMA divergence to pass threshold)
        MarketView {
            symbol: "TEST",
            last: Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot {
                ema_fast: 101.0,  // 1% above slow (> 0.5% threshold)
                ema_slow: 100.0,
                z_momentum: z_mom,
                z_stretch,
                z_vol,
                vol: 100.0,
                vol_mean: 100.0,
                z_volume_spike: 0.5,
                ..Default::default()
            },
            aux: MarketAux::default(),
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
    fn test_momentum_strategy_bullish() {
        let mut strat = MomentumStrategy {
            config: StrategyConfig { entry_threshold: 0.2, ..Default::default() },
            momentum_threshold: 0.5,
            require_trend_align: true,
        };
        let market = make_market(2.0, 0.0, 1.0); // Strong positive momentum + uptrend
        let mut state = make_state();
        let action = strat.update(market, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Expected Buy, got {:?}", action);
    }

    #[test]
    fn test_momentum_strategy_bearish() {
        let mut strat = MomentumStrategy {
            config: StrategyConfig { entry_threshold: 0.2, ..Default::default() },
            momentum_threshold: 0.5,
            require_trend_align: true,
        };
        // Need downtrend for bearish signal with trend alignment
        let mut market = make_market(-2.0, 0.0, 1.0);
        market.indicators.ema_fast = 98.0; // Stronger downtrend
        market.indicators.ema_slow = 100.0;
        let mut state = make_state();
        let action = strat.update(market, &mut state);
        assert!(matches!(action, Action::Sell { .. }), "Expected Sell, got {:?}", action);
    }

    #[test]
    fn test_mean_reversion_buy_dip() {
        let mut strat = MeanReversionStrategy {
            config: StrategyConfig { entry_threshold: 0.2, ..Default::default() },
            stretch_threshold: 2.0,
            require_trend_align: true,
        };
        // Uptrend + stretched below = buy dip
        let mut market = make_market(0.0, -2.5, 0.8);
        market.indicators.ema_fast = 102.0; // Stronger uptrend
        market.indicators.ema_slow = 100.0;
        market.indicators.vol = 80.0;  // Normal vol (not high)
        market.indicators.vol_mean = 100.0;
        let mut state = make_state();
        let action = strat.update(market, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Expected Buy, got {:?}", action);
    }

    #[test]
    fn test_adaptive_low_vol_momentum() {
        let mut strat = AdaptiveStrategy {
            config: StrategyConfig { entry_threshold: 0.2, ..Default::default() },
            momentum_threshold: 0.5,
            reversion_threshold: 2.0,
        };
        // Low vol regime should use momentum
        let mut market = make_market(1.5, 0.0, -1.0);
        market.indicators.vol = 40.0; // Low vol
        market.indicators.vol_mean = 100.0;
        market.indicators.ema_fast = 101.0; // Uptrend needed
        market.indicators.ema_slow = 100.0;
        let mut state = make_state();
        let action = strat.update(market, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Expected Buy, got {:?}", action);
    }
}
