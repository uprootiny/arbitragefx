use crate::strategy::{Action, StrategyState};
use crate::state::Config;

pub struct RiskEngine {
    cfg: Config,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{MetricsState, PortfolioState};

    fn make_config() -> Config {
        let mut cfg = Config::from_env();
        cfg.max_daily_loss_pct = 0.05;    // 5% max daily loss
        cfg.max_position_pct = 0.10;       // 10% max position
        cfg.cooldown_secs = 60;
        cfg.max_trades_per_day = 20;
        cfg.kill_file = "/tmp/nonexistent_kill_file".to_string();
        cfg
    }

    fn make_state(position: f64, entry_price: f64, equity: f64, realized_pnl: f64) -> StrategyState {
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
        assert!(matches!(action, Action::Close), "Expected Close, got {:?}", action);
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
        assert!(matches!(action, Action::Close), "Expected Close at limit, got {:?}", action);
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
        assert!(matches!(action, Action::Buy { .. }), "Expected Buy allowed, got {:?}", action);
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
        assert!(matches!(action, Action::Hold), "Expected Hold due to exposure, got {:?}", action);
    }

    #[test]
    fn test_exposure_limit_allows_risk_reduction() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        // Over-exposed long position
        let state = make_state(2.0, 1000.0, 10000.0, 0.0);

        // Close should be allowed
        let close_action = engine.apply_with_price(&state, Action::Close, 1000, 1000.0);
        assert!(matches!(close_action, Action::Close), "Close should be allowed");

        // Sell should be allowed (reduces long exposure)
        let sell_action = engine.apply_with_price(&state, Action::Sell { qty: 0.5 }, 1000, 1000.0);
        assert!(matches!(sell_action, Action::Sell { .. }), "Sell should be allowed");
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
        assert!(matches!(action, Action::Close), "Expected Close on short loss, got {:?}", action);
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
        assert!(matches!(action, Action::Buy { .. }), "Expected Buy allowed, got {:?}", action);
    }

    #[test]
    fn test_cooldown_after_loss() {
        let cfg = make_config();
        let mut engine = RiskEngine::new(cfg);

        let mut state = make_state(0.0, 0.0, 10000.0, -100.0);
        state.last_loss_ts = 950;  // Loss was 50 seconds ago

        // Try to trade at ts=1000, cooldown is 60s
        let action = engine.apply_with_price(&state, Action::Buy { qty: 0.1 }, 1000, 50000.0);

        // Should block due to cooldown
        assert!(matches!(action, Action::Hold), "Expected Hold during cooldown, got {:?}", action);
    }
}

impl RiskEngine {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
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
    pub fn apply_with_price(&mut self, state: &StrategyState, action: Action, now_ts: u64, current_price: f64) -> Action {
        if state.trading_halted {
            return Action::Hold;
        }
        if std::path::Path::new(&self.cfg.kill_file).exists() {
            return Action::Hold;
        }
        if now_ts.saturating_sub(state.last_loss_ts) < self.cfg.cooldown_secs {
            return Action::Hold;
        }
        if state.trades_today >= self.cfg.max_trades_per_day {
            return Action::Hold;
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
                Action::Sell { qty } if state.portfolio.position > 0.0 => return Action::Sell { qty },
                Action::Buy { qty } if state.portfolio.position < 0.0 => return Action::Buy { qty },
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
