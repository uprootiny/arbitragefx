use crate::strategy::StrategyState;

pub struct MetricsEngine;

impl MetricsEngine {
    pub fn new() -> Self {
        Self
    }

    /// Update metrics with mark-to-market price
    pub fn update_with_price(&mut self, state: &mut StrategyState, mark_price: f64) {
        // Mark to market: equity = cash + position * current_price
        state.portfolio.equity = state.portfolio.cash + state.portfolio.position * mark_price;

        let equity = state.portfolio.equity;
        if equity > state.metrics.equity_peak {
            state.metrics.equity_peak = equity;
        }
        let drawdown = if state.metrics.equity_peak > 0.0 {
            (equity - state.metrics.equity_peak) / state.metrics.equity_peak
        } else {
            0.0
        };
        if drawdown < state.metrics.max_drawdown {
            state.metrics.max_drawdown = drawdown;
        }
    }

    pub fn update(&mut self, state: &mut StrategyState) {
        let equity = state.portfolio.equity;
        if equity > state.metrics.equity_peak {
            state.metrics.equity_peak = equity;
        }
        let drawdown = if state.metrics.equity_peak > 0.0 {
            (equity - state.metrics.equity_peak) / state.metrics.equity_peak
        } else {
            0.0
        };
        if drawdown < state.metrics.max_drawdown {
            state.metrics.max_drawdown = drawdown;
        }
    }
}
