use crate::state::Config;
use crate::strategy::{Action, StrategyState};

/// Kelly criterion position sizing
pub fn kelly_size(win_rate: f64, avg_win: f64, avg_loss: f64, equity: f64, fraction: f64) -> f64 {
    if avg_loss <= 0.0 || win_rate <= 0.0 || win_rate >= 1.0 {
        return 0.0;
    }
    let win_loss_ratio = avg_win / avg_loss;
    let kelly = win_rate - (1.0 - win_rate) / win_loss_ratio;
    if kelly <= 0.0 {
        return 0.0; // Negative edge, don't trade
    }
    (kelly * fraction * equity).max(0.0)
}

/// Calculate expectancy per trade
pub fn expectancy(win_rate: f64, avg_win: f64, avg_loss: f64) -> f64 {
    (win_rate * avg_win) - ((1.0 - win_rate) * avg_loss)
}

/// Risk of ruin calculation (simplified)
pub fn risk_of_ruin(win_rate: f64, risk_per_trade: f64, account_units: f64) -> f64 {
    if win_rate >= 1.0 || win_rate <= 0.0 || risk_per_trade <= 0.0 {
        return 0.0;
    }
    let loss_rate = 1.0 - win_rate;
    let ratio = loss_rate / win_rate;
    ratio.powf(account_units / risk_per_trade)
}

/// Fill probability for limit orders (adverse selection model)
pub fn limit_fill_probability(
    limit_price: f64,
    market_price: f64,
    volatility: f64,
    is_buy: bool,
) -> f64 {
    if volatility <= 0.0 {
        return 0.5;
    }
    let distance = if is_buy {
        (market_price - limit_price) / market_price // How far below market
    } else {
        (limit_price - market_price) / market_price // How far above market
    };

    if distance < 0.0 {
        // Limit is worse than market, fills instantly but why would you?
        return 1.0;
    }

    // Exponential decay based on distance relative to volatility
    let base_prob = (-distance / volatility).exp();
    // Adverse selection discount: fills that happen tend to be losers
    (base_prob * 0.7).min(1.0).max(0.0)
}

pub struct RiskEngine {
    cfg: Config,
    // Track for Kelly sizing
    recent_wins: u64,
    recent_losses: u64,
    total_win_amount: f64,
    total_loss_amount: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{MetricsState, PortfolioState};

    #[test]
    fn test_kelly_size_positive_edge() {
        // 60% win rate, 1.5:1 reward ratio
        let size = kelly_size(0.6, 1.5, 1.0, 10000.0, 0.25);
        assert!(size > 0.0, "Should recommend positive size");
        assert!(size < 10000.0 * 0.25, "Should be less than 25% of equity");
    }

    #[test]
    fn test_kelly_size_negative_edge() {
        // 40% win rate, 1:1 reward ratio = negative expectancy
        let size = kelly_size(0.4, 1.0, 1.0, 10000.0, 0.25);
        assert_eq!(size, 0.0, "Should not trade with negative edge");
    }

    #[test]
    fn test_expectancy_calculation() {
        // 55% win, avg win $2, avg loss $1
        let exp = expectancy(0.55, 2.0, 1.0);
        // Expected = 0.55 * 2 - 0.45 * 1 = 1.1 - 0.45 = 0.65
        assert!((exp - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_risk_of_ruin() {
        // 55% win rate, 5% risk per trade, 20 units of account
        let ror = risk_of_ruin(0.55, 0.05, 1.0);
        assert!(ror < 1.0, "Risk of ruin should be < 100%");
        assert!(ror > 0.0, "Risk of ruin should be > 0%");
    }

    #[test]
    fn test_limit_fill_probability() {
        // Buy limit at market = immediate fill (high probability)
        // Buy limit below market = only fills if price drops (lower probability)
        let prob_at = limit_fill_probability(100.0, 100.0, 0.02, true);
        let prob_below = limit_fill_probability(99.0, 100.0, 0.02, true);
        assert!(
            prob_at > prob_below,
            "Limit at market should fill more often than below"
        );
        assert!(
            prob_below > 0.0,
            "Limit below should still have some fill probability"
        );
        assert!(prob_below < 1.0, "Limit below should not guarantee fill");
    }

    #[test]
    fn test_risk_engine_kelly_tracking() {
        let cfg = Config::from_env();
        let mut engine = RiskEngine::new(cfg);

        // Record enough trades (minimum 5 for expectancy calculation)
        engine.record_trade(10.0); // Win
        engine.record_trade(10.0); // Win
        engine.record_trade(-5.0); // Loss
        engine.record_trade(10.0); // Win
        engine.record_trade(10.0); // Win
        engine.record_trade(-5.0); // Loss

        // 4 wins, 2 losses = 66.7% win rate
        // Avg win = 10, avg loss = 5, ratio = 2
        // Expectancy = 0.667 * 10 - 0.333 * 5 = 6.67 - 1.67 = 5.0
        let exp = engine.current_expectancy();
        assert!(
            exp > 0.0,
            "Expectancy should be positive with 4W/2L at 2:1 ratio"
        );
    }

    fn make_config() -> Config {
        let mut cfg = Config::from_env();
        cfg.max_daily_loss_pct = 0.05; // 5% max daily loss
        cfg.max_position_pct = 0.10; // 10% max position
        cfg.cooldown_secs = 60;
        cfg.max_trades_per_day = 20;
        cfg.kill_file = "/tmp/nonexistent_kill_file".to_string();
        cfg
    }

    fn make_state(
        position: f64,
        entry_price: f64,
        equity: f64,
        realized_pnl: f64,
    ) -> StrategyState {
        StrategyState {
            portfolio: PortfolioState {
                cash: equity - (position * entry_price),
                position,
                entry_price,
                equity,
            },
            metrics: MetricsState {
                pnl: realized_pnl,
                ..Default::default()
            },
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        }
    }

    #[test]
    fn test_unrealized_loss_triggers_close() {
        // CRITICAL: Tests that unrealized loss (not just realized) triggers risk limit
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Long 0.1 BTC at 50000 (5000 notional), equity 100000 (5% exposure)
        // Unrealized loss at 45000 = 0.1 * -5000 = -500 = 0.5% of equity
        // But we need 5% loss to trigger, so use larger position or smaller equity
        // Use: 1 BTC at 50000, equity 100000 (50% exposure but we need the loss test)
        let state = make_state(0.1, 50000.0, 10000.0, 0.0);

        // Price drops to 45000 → unrealized loss = 0.1 * -5000 = -500 = 5% of 10000
        // This meets 5% max_daily_loss_pct threshold
        let action = engine.apply_with_price(&state, Action::Hold, 1000, 45000.0);

        // Should force close due to unrealized loss
        assert!(
            matches!(action, Action::Close),
            "Expected Close, got {:?}",
            action
        );
    }

    #[test]
    fn test_realized_plus_unrealized_triggers_close() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Already have -200 realized loss (2% of 10000), small long position
        let state = make_state(0.1, 50000.0, 10000.0, -200.0);

        // Price drops to 47000 → unrealized = 0.1 * -3000 = -300 (3%)
        // Total = -500 = 5% = exactly at limit
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.01 }, 1000, 47000.0);

        // Should force close
        assert!(
            matches!(action, Action::Close),
            "Expected Close at limit, got {:?}",
            action
        );
    }

    #[test]
    fn test_unrealized_gain_offsets_realized_loss() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // -300 realized loss, but small position is in profit
        // equity = 100000, position = 0.01 BTC at 50000 = 500 notional = 0.5% exposure
        let state = make_state(0.01, 50000.0, 100000.0, -300.0);

        // Price at 50500 → unrealized = 0.01 * 500 = +5
        // Total = -300 + 5 = -295 (still loss but under 5% of 100k)
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.001 }, 1000, 50500.0);

        // Should allow trading (within limits)
        assert!(
            matches!(action, Action::Buy { .. }),
            "Expected Buy allowed, got {:?}",
            action
        );
    }

    #[test]
    fn test_exposure_limit_blocks_new_positions() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Already have position worth 15% of equity
        // position * price / equity = 1.5 * 1000 / 10000 = 15%
        let state = make_state(1.5, 1000.0, 10000.0, 0.0);

        // Try to buy more
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.5 }, 1000, 1000.0);

        // Should block (over 10% limit)
        assert!(
            matches!(action, Action::Hold),
            "Expected Hold due to exposure, got {:?}",
            action
        );
    }

    #[test]
    fn test_exposure_limit_allows_risk_reduction() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Over-exposed long position
        let state = make_state(2.0, 1000.0, 10000.0, 0.0);

        // Close should be allowed
        let close_action = engine.apply_with_price(&state, Action::Close, 1000, 1000.0);
        assert!(
            matches!(close_action, Action::Close),
            "Close should be allowed"
        );

        // Sell should be allowed (reduces long exposure)
        let sell_action = engine.apply_with_price(&state, Action::Sell { qty: 0.5 }, 1000, 1000.0);
        assert!(
            matches!(sell_action, Action::Sell { .. }),
            "Sell should be allowed"
        );
    }

    #[test]
    fn test_short_position_unrealized_loss() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Short 0.1 BTC at 50000
        let state = make_state(-0.1, 50000.0, 10000.0, 0.0);

        // Price rises to 55000 → unrealized loss = -0.1 * 5000 = -500 = 5%
        let action = engine.apply_with_price(&state, Action::Hold, 1000, 55000.0);

        // Should force close
        assert!(
            matches!(action, Action::Close),
            "Expected Close on short loss, got {:?}",
            action
        );
    }

    #[test]
    fn test_no_position_no_unrealized() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // No position
        let state = make_state(0.0, 0.0, 10000.0, -400.0);

        // 4% realized loss, below 5% limit
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.1 }, 1000, 50000.0);

        // Should allow new trade
        assert!(
            matches!(action, Action::Buy { .. }),
            "Expected Buy allowed, got {:?}",
            action
        );
    }

    #[test]
    fn test_cooldown_after_loss() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        let mut state = make_state(0.0, 0.0, 10000.0, -100.0);
        state.last_loss_ts = 950; // Loss was 50 seconds ago

        // Try to trade at ts=1000, cooldown is 60s
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.1 }, 1000, 50000.0);

        // Should block due to cooldown
        assert!(
            matches!(action, Action::Hold),
            "Expected Hold during cooldown, got {:?}",
            action
        );
    }
}

impl RiskEngine {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            recent_wins: 0,
            recent_losses: 0,
            total_win_amount: 0.0,
            total_loss_amount: 0.0,
        }
    }

    /// Record a trade result for Kelly sizing
    pub fn record_trade(&mut self, pnl: f64) {
        if pnl > 0.0 {
            self.recent_wins += 1;
            self.total_win_amount += pnl;
        } else if pnl < 0.0 {
            self.recent_losses += 1;
            self.total_loss_amount += pnl.abs();
        }
    }

    /// Get recommended position size based on Kelly criterion
    pub fn kelly_position_size(&self, equity: f64) -> f64 {
        let total_trades = self.recent_wins + self.recent_losses;
        if total_trades < 10 {
            // Not enough data, use fixed small size
            return equity * 0.01; // 1% of equity
        }
        let win_rate = self.recent_wins as f64 / total_trades as f64;
        let avg_win = if self.recent_wins > 0 {
            self.total_win_amount / self.recent_wins as f64
        } else {
            0.0
        };
        let avg_loss = if self.recent_losses > 0 {
            self.total_loss_amount / self.recent_losses as f64
        } else {
            1.0 // Avoid division by zero
        };
        kelly_size(win_rate, avg_win, avg_loss, equity, 0.25) // 1/4 Kelly
    }

    /// Get current expectancy per trade
    pub fn current_expectancy(&self) -> f64 {
        let total_trades = self.recent_wins + self.recent_losses;
        if total_trades < 5 {
            return 0.0;
        }
        let win_rate = self.recent_wins as f64 / total_trades as f64;
        let avg_win = if self.recent_wins > 0 {
            self.total_win_amount / self.recent_wins as f64
        } else {
            0.0
        };
        let avg_loss = if self.recent_losses > 0 {
            self.total_loss_amount / self.recent_losses as f64
        } else {
            0.0
        };
        expectancy(win_rate, avg_win, avg_loss)
    }

    /// Calculate unrealized PnL for current position
    fn unrealized_pnl(state: &StrategyState, current_price: f64) -> f64 {
        if state.portfolio.position == 0.0 || state.portfolio.entry_price == 0.0 {
            return 0.0;
        }
        let price_delta = current_price - state.portfolio.entry_price;
        state.portfolio.position * price_delta
    }

    /// Calculate total exposure as fraction of equity
    fn exposure_pct(state: &StrategyState, current_price: f64) -> f64 {
        let notional = state.portfolio.position.abs() * current_price;
        notional / state.portfolio.equity.max(1.0)
    }

    pub fn apply(&mut self, state: &StrategyState, action: Action, now_ts: u64) -> Action {
        self.apply_with_price(state, action, now_ts, state.portfolio.entry_price)
    }

    /// Apply risk checks with current market price for MTM calculations
    pub fn apply_with_price(
        &mut self,
        state: &StrategyState,
        action: Action,
        now_ts: u64,
        current_price: f64,
    ) -> Action {
        if state.trading_halted {
            return match action {
                Action::Close => Action::Close,
                _ => Action::Hold,
            };
        }
        if std::path::Path::new(&self.cfg.kill_file).exists() {
            return match action {
                Action::Close => Action::Close,
                _ => Action::Hold,
            };
        }
        if now_ts.saturating_sub(state.last_loss_ts) < self.cfg.cooldown_secs {
            return match action {
                Action::Close => Action::Close,
                _ => Action::Hold,
            };
        }
        if state.trades_today >= self.cfg.max_trades_per_day {
            return match action {
                Action::Close => Action::Close,
                Action::Sell { qty } if state.portfolio.position > 0.0 => Action::Sell { qty },
                Action::Buy { qty } if state.portfolio.position < 0.0 => Action::Buy { qty },
                _ => Action::Hold,
            };
        }

        // FIXED: Check both realized AND unrealized loss
        let unrealized = Self::unrealized_pnl(state, current_price);
        let total_pnl = state.metrics.pnl + unrealized;

        if total_pnl < 0.0 {
            let loss_pct = total_pnl.abs() / state.portfolio.equity.max(1.0);
            if loss_pct >= self.cfg.max_daily_loss_pct {
                // If we have a position and are over daily loss, force close
                if state.portfolio.position != 0.0 {
                    return Action::Close;
                }
                return Action::Hold;
            }
        }

        // Check exposure limit
        let exposure = Self::exposure_pct(state, current_price);
        if exposure > self.cfg.max_position_pct {
            // Over-exposed: only allow risk-reducing actions
            match action {
                Action::Close => return action,
                Action::Sell { qty } if state.portfolio.position > 0.0 => {
                    return Action::Sell { qty }
                }
                Action::Buy { qty } if state.portfolio.position < 0.0 => {
                    return Action::Buy { qty }
                }
                _ => return Action::Hold,
            }
        }

        if state.portfolio.position != 0.0 {
            match action {
                Action::Close => action,
                Action::Sell { .. } => action,
                Action::Buy { .. } => action,
                _ => Action::Hold,
            }
        } else {
            action
        }
    }
}
