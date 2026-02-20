use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub ts: u64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

#[derive(Debug, Clone)]
pub struct MarketState {
    pub last: Candle,
    pub window: VecDeque<f64>,
    pub window_size: usize,
}

impl MarketState {
    pub fn new(window_size: usize) -> Self {
        Self {
            last: Candle {
                ts: 0,
                o: 0.0,
                h: 0.0,
                l: 0.0,
                c: 0.0,
                v: 0.0,
            },
            window: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    pub fn update(&mut self, candle: Candle) {
        self.last = candle;
        self.window.push_back(candle.c);
        if self.window.len() > self.window_size {
            self.window.pop_front();
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Portfolio {
    pub cash: f64,
    pub position: f64,
    pub entry_price: f64,
    pub equity: f64,
}

impl Portfolio {
    pub fn new(cash: f64) -> Self {
        Self {
            cash,
            position: 0.0,
            entry_price: 0.0,
            equity: cash,
        }
    }

    pub fn apply_fill(&mut self, price: f64, qty: f64, fee: f64) {
        if qty == 0.0 {
            return;
        }
        let prev_pos = self.position;
        let new_pos = prev_pos + qty;
        if prev_pos == 0.0 || (prev_pos > 0.0) == (new_pos > 0.0) {
            let total = prev_pos.abs() + qty.abs();
            if total > 0.0 {
                self.entry_price = (self.entry_price * prev_pos.abs() + price * qty.abs()) / total;
            }
        } else if new_pos != 0.0 {
            self.entry_price = price;
        } else {
            self.entry_price = 0.0;
        }
        self.cash -= qty * price;
        self.cash -= fee;
        self.position = new_pos;
        self.equity = self.cash + self.position * price;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RiskState {
    pub halted: bool,
    pub last_trade_ts: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct StrategyState {
    pub last_signal_ts: u64,
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub market: MarketState,
    pub portfolio: Portfolio,
    pub risk: RiskState,
    pub strategy: StrategyState,
}

impl EngineState {
    pub fn new(window_size: usize, cash: f64) -> Self {
        Self {
            market: MarketState::new(window_size),
            portfolio: Portfolio::new(cash),
            risk: RiskState {
                halted: false,
                last_trade_ts: 0,
            },
            strategy: StrategyState { last_signal_ts: 0 },
        }
    }
}
