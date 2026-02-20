use super::state::{Candle, EngineState};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Hold,
    Buy { qty: f64 },
    Sell { qty: f64 },
    Close,
}

pub trait Strategy {
    fn id(&self) -> &'static str;
    fn decide(&mut self, state: &EngineState) -> Action;
}

#[derive(Debug, Default)]
pub struct SimpleMomentum {
    pub qty: f64,
    pub threshold: f64,
}

impl Strategy for SimpleMomentum {
    fn id(&self) -> &'static str {
        "simple_momentum"
    }

    fn decide(&mut self, state: &EngineState) -> Action {
        let last = state.market.last.c;
        if state.market.window.len() < 2 {
            return Action::Hold;
        }
        let prev = state.market.window[state.market.window.len() - 2];
        let move_pct = if prev > 0.0 { (last / prev) - 1.0 } else { 0.0 };
        if move_pct > self.threshold {
            Action::Buy { qty: self.qty }
        } else if move_pct < -self.threshold {
            Action::Sell { qty: self.qty }
        } else {
            Action::Hold
        }
    }
}

pub fn default_candle() -> Candle {
    Candle {
        ts: 0,
        o: 0.0,
        h: 0.0,
        l: 0.0,
        c: 0.0,
        v: 0.0,
    }
}
