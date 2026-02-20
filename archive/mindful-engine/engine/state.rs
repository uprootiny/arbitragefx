//! Engine state with deterministic hashing for replay validation.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::events::{Timestamp, TradeSide};
use super::narrative_detector::{NarrativeIndicators, NarrativeRegime};

/// Complete engine state - hashable for replay validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    /// Current logical time
    pub now: Timestamp,

    /// Per-symbol market state
    pub symbols: HashMap<String, SymbolState>,

    /// Global portfolio
    pub portfolio: Portfolio,

    /// Open orders by client_id
    pub orders: HashMap<String, OrderState>,

    /// Risk state
    pub risk: RiskState,

    /// Strategy states by id
    pub strategies: HashMap<String, EngineStrategyState>,

    /// Is system halted
    pub halted: bool,
    pub halt_reason: Option<String>,

    /// Sequence number for determinism
    pub seq: u64,

    /// Current market regime (Right Mindfulness)
    pub regime: RegimeState,
}

/// Market regime state for Right Mindfulness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeState {
    /// Current regime classification
    pub current: NarrativeRegime,
    /// Last update timestamp
    pub last_update_ts: Timestamp,
    /// Narrative score (0.0 = grounded, 1.0 = reflexive)
    pub narrative_score: f64,
    /// Position size multiplier based on regime
    pub position_multiplier: f64,
    /// Is regime stale (hasn't been updated recently)?
    pub is_stale: bool,
    /// Bars since last regime change
    pub bars_in_regime: u32,
}

impl Default for RegimeState {
    fn default() -> Self {
        Self {
            current: NarrativeRegime::Grounded,
            last_update_ts: 0,
            narrative_score: 0.0,
            position_multiplier: 1.0,
            is_stale: true, // Start stale until first update
            bars_in_regime: 0,
        }
    }
}

impl RegimeState {
    /// Update regime from narrative indicators
    pub fn update(&mut self, indicators: &NarrativeIndicators, ts: Timestamp) {
        let new_regime = indicators.regime();
        let new_score = indicators.narrative_score();

        // Track regime changes
        if new_regime != self.current {
            self.bars_in_regime = 0;
        } else {
            self.bars_in_regime += 1;
        }

        self.current = new_regime;
        self.narrative_score = new_score;
        self.position_multiplier = new_regime.position_multiplier();
        self.last_update_ts = ts;
        self.is_stale = false;
    }

    /// Check if regime is stale
    pub fn check_staleness(&mut self, now: Timestamp, max_age_ms: u64) {
        self.is_stale = now.saturating_sub(self.last_update_ts) > max_age_ms;
    }

    /// Get effective position multiplier (conservative if stale)
    pub fn effective_multiplier(&self) -> f64 {
        if self.is_stale {
            0.5 // Conservative when stale
        } else {
            self.position_multiplier
        }
    }
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            now: 0,
            symbols: HashMap::new(),
            portfolio: Portfolio::new(10000.0), // default starting capital
            orders: HashMap::new(),
            risk: RiskState::default(),
            strategies: HashMap::new(),
            halted: false,
            halt_reason: None,
            seq: 0,
            regime: RegimeState::default(),
        }
    }

    /// Compute deterministic state hash for replay validation
    pub fn hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut h = DefaultHasher::new();

        // Core state components
        self.now.hash(&mut h);
        self.seq.hash(&mut h);
        self.halted.hash(&mut h);

        // Portfolio (quantized to avoid float comparison issues)
        ((self.portfolio.cash * 1e8) as i64).hash(&mut h);
        ((self.portfolio.equity * 1e8) as i64).hash(&mut h);

        // Positions
        let mut positions: Vec<_> = self.portfolio.positions.iter().collect();
        positions.sort_by_key(|(k, _)| *k);
        for (sym, pos) in positions {
            sym.hash(&mut h);
            ((pos.qty * 1e8) as i64).hash(&mut h);
            ((pos.entry_price * 1e8) as i64).hash(&mut h);
        }

        // Open orders
        let mut orders: Vec<_> = self.orders.keys().collect();
        orders.sort();
        for id in orders {
            id.hash(&mut h);
        }

        // Risk state
        self.risk.trades_today.hash(&mut h);
        ((self.risk.daily_pnl * 1e8) as i64).hash(&mut h);
        self.risk.consecutive_errors.hash(&mut h);

        h.finish()
    }

    /// Get or create symbol state
    pub fn symbol_mut(&mut self, symbol: &str) -> &mut SymbolState {
        self.symbols.entry(symbol.to_string()).or_insert_with(SymbolState::new)
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-symbol market state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolState {
    /// Last candle data
    pub last_price: f64,
    pub last_ts: Timestamp,

    /// Indicators
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub vwap: f64,
    pub volatility: f64,

    /// Rolling stats (Welford)
    pub price_n: u64,
    pub price_mean: f64,
    pub price_m2: f64,

    /// Auxiliary data
    pub funding_rate: f64,
    pub liquidation_score: f64,
    pub spread: f64,

    /// Data freshness
    pub candle_count: u64,
    pub last_trade_ts: Timestamp,

    // === Additional indicators for non-grasping signals ===
    /// Previous candle prices for rate-of-change
    pub prev_close: f64,
    pub prev_prev_close: f64,

    /// Session high/low for range context
    pub session_high: f64,
    pub session_low: f64,

    /// Short-term RSI components
    pub gain_ema: f64,
    pub loss_ema: f64,
}

impl SymbolState {
    pub fn new() -> Self {
        Self {
            last_price: 0.0,
            last_ts: 0,
            ema_fast: 0.0,
            ema_slow: 0.0,
            vwap: 0.0,
            volatility: 0.0,
            price_n: 0,
            price_mean: 0.0,
            price_m2: 0.0,
            funding_rate: 0.0,
            liquidation_score: 0.0,
            spread: 0.0,
            candle_count: 0,
            last_trade_ts: 0,
            prev_close: 0.0,
            prev_prev_close: 0.0,
            session_high: 0.0,
            session_low: f64::MAX,
            gain_ema: 0.0,
            loss_ema: 0.0,
        }
    }

    /// Update indicators on new candle
    pub fn on_candle(&mut self, ts: Timestamp, close: f64, _volume: f64, alpha_fast: f64, alpha_slow: f64) {
        // Preserve history before update
        self.prev_prev_close = self.prev_close;
        self.prev_close = self.last_price;

        self.last_price = close;
        self.last_ts = ts;
        self.candle_count += 1;

        // Session high/low
        self.session_high = self.session_high.max(close);
        if self.session_low == f64::MAX {
            self.session_low = close;
        } else {
            self.session_low = self.session_low.min(close);
        }

        // EMA
        if self.ema_fast == 0.0 {
            self.ema_fast = close;
            self.ema_slow = close;
        } else {
            self.ema_fast = alpha_fast * close + (1.0 - alpha_fast) * self.ema_fast;
            self.ema_slow = alpha_slow * close + (1.0 - alpha_slow) * self.ema_slow;
        }

        // Welford online variance
        self.price_n += 1;
        let delta = close - self.price_mean;
        self.price_mean += delta / self.price_n as f64;
        let delta2 = close - self.price_mean;
        self.price_m2 += delta * delta2;

        if self.price_n > 1 {
            self.volatility = (self.price_m2 / (self.price_n as f64 - 1.0)).sqrt();
        }

        // RSI components (gain/loss EMA) - alpha ~0.07 for 14-period
        if self.prev_close > 0.0 {
            let change = close - self.prev_close;
            let gain = if change > 0.0 { change } else { 0.0 };
            let loss = if change < 0.0 { -change } else { 0.0 };

            let rsi_alpha = 1.0 / 14.0;
            if self.gain_ema == 0.0 && self.loss_ema == 0.0 {
                self.gain_ema = gain;
                self.loss_ema = loss;
            } else {
                self.gain_ema = rsi_alpha * gain + (1.0 - rsi_alpha) * self.gain_ema;
                self.loss_ema = rsi_alpha * loss + (1.0 - rsi_alpha) * self.loss_ema;
            }
        }
    }

    /// Z-score of current momentum (EMA diff) - LAGGING, use with caution
    pub fn z_momentum(&self) -> f64 {
        if self.volatility > 0.0 {
            (self.ema_fast - self.ema_slow) / self.volatility
        } else {
            0.0
        }
    }

    /// RSI (0-100) - measures exhaustion, not direction
    /// Low RSI = oversold (potential mean reversion up)
    /// High RSI = overbought (potential mean reversion down)
    pub fn rsi(&self) -> f64 {
        if self.loss_ema < 1e-9 {
            if self.gain_ema > 1e-9 { 100.0 } else { 50.0 }
        } else {
            100.0 - (100.0 / (1.0 + self.gain_ema / self.loss_ema))
        }
    }

    /// Z-score of price relative to mean - for mean reversion
    /// Negative = price below mean (potential buy on reversion)
    /// Positive = price above mean (potential sell on reversion)
    pub fn z_mean_deviation(&self) -> f64 {
        if self.volatility > 0.0 && self.price_n > 5 {
            (self.last_price - self.price_mean) / self.volatility
        } else {
            0.0
        }
    }

    /// Price position in session range (0-1)
    /// Near 0 = near session low (potential support)
    /// Near 1 = near session high (potential resistance)
    pub fn range_position(&self) -> f64 {
        let range = self.session_high - self.session_low;
        if range > 0.0 {
            (self.last_price - self.session_low) / range
        } else {
            0.5
        }
    }

    /// Momentum acceleration (2nd derivative) - LEADING indicator
    /// Positive = momentum accelerating
    /// Negative = momentum decelerating (reversal signal)
    pub fn momentum_acceleration(&self) -> f64 {
        if self.prev_close > 0.0 && self.prev_prev_close > 0.0 {
            let roc_now = (self.last_price - self.prev_close) / self.prev_close;
            let roc_prev = (self.prev_close - self.prev_prev_close) / self.prev_prev_close;
            roc_now - roc_prev  // Acceleration
        } else {
            0.0
        }
    }

    /// Composite signal for mean-reversion (non-grasping aligned)
    /// Positive = buy signal (oversold, below mean, funding pressure)
    /// Negative = sell signal (overbought, above mean, funding pressure)
    ///
    /// Philosophy: Don't chase strength; provide liquidity to exhaustion.
    pub fn mean_reversion_score(&self) -> f64 {
        // RSI component: fade extremes
        let rsi = self.rsi();
        let rsi_signal = if rsi < 30.0 {
            (30.0 - rsi) / 30.0  // Oversold -> buy
        } else if rsi > 70.0 {
            -(rsi - 70.0) / 30.0  // Overbought -> sell
        } else {
            0.0
        };

        // Mean deviation: fade deviations
        let z_dev = self.z_mean_deviation();
        let mean_signal = -z_dev * 0.2;  // Price above mean -> lean sell

        // Funding pressure: fade crowded trades
        // High positive funding = longs paying, crowd is long -> fade (sell)
        let funding_signal = -self.funding_rate * 500.0;  // Amplify small funding rates

        // Combine with weights
        rsi_signal * 0.5 + mean_signal * 0.3 + funding_signal * 0.2
    }

    /// Is data stale?
    pub fn is_stale(&self, now: Timestamp, max_age_ms: u64) -> bool {
        now.saturating_sub(self.last_ts) > max_age_ms
    }
}

impl Default for SymbolState {
    fn default() -> Self {
        Self::new()
    }
}

/// Portfolio state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub cash: f64,
    pub equity: f64,
    pub equity_peak: f64,
    pub positions: HashMap<String, Position>,
    pub realized_pnl: f64,
}

impl Portfolio {
    pub fn new(starting_cash: f64) -> Self {
        Self {
            cash: starting_cash,
            equity: starting_cash,
            equity_peak: starting_cash,
            positions: HashMap::new(),
            realized_pnl: 0.0,
        }
    }

    /// Apply a fill to portfolio
    pub fn apply_fill(&mut self, symbol: &str, side: TradeSide, qty: f64, price: f64, fee: f64) -> f64 {
        let signed_qty = match side {
            TradeSide::Buy => qty,
            TradeSide::Sell => -qty,
        };

        let pos = self.positions.entry(symbol.to_string()).or_insert(Position::default());
        let mut realized = 0.0;

        // Closing logic
        let prev_qty = pos.qty;
        if prev_qty != 0.0 && (prev_qty + signed_qty).abs() < prev_qty.abs() {
            let close_qty = signed_qty.abs().min(prev_qty.abs());
            let dir = if prev_qty > 0.0 { 1.0 } else { -1.0 };
            realized = (price - pos.entry_price) * close_qty * dir;
        }

        // Update position
        let cost = price * qty;
        self.cash -= cost + fee;
        pos.qty += signed_qty;

        if pos.qty.abs() > 1e-9 {
            pos.entry_price = price;
        } else {
            pos.qty = 0.0;
            pos.entry_price = 0.0;
        }

        // Update equity
        self.equity = self.cash + self.positions.values()
            .map(|p| p.qty * p.entry_price)
            .sum::<f64>();

        self.equity_peak = self.equity_peak.max(self.equity);
        self.realized_pnl += realized;

        realized
    }

    /// Current drawdown percentage
    pub fn drawdown_pct(&self) -> f64 {
        if self.equity_peak > 0.0 {
            (self.equity_peak - self.equity) / self.equity_peak
        } else {
            0.0
        }
    }

    /// Total position value
    pub fn total_exposure(&self, prices: &HashMap<String, f64>) -> f64 {
        self.positions.iter()
            .map(|(sym, pos)| {
                let price = prices.get(sym).copied().unwrap_or(pos.entry_price);
                pos.qty.abs() * price
            })
            .sum()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Position {
    pub qty: f64,
    pub entry_price: f64,
}

/// Order tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderState {
    pub client_id: String,
    pub symbol: String,
    pub side: TradeSide,
    pub qty: f64,
    pub filled_qty: f64,
    pub price: Option<f64>,
    pub status: OrderStatus,
    pub created_ts: Timestamp,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Acked,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
}

/// Risk state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskState {
    pub trades_today: u32,
    pub trade_day: u64,
    pub daily_pnl: f64,
    pub last_trade_ts: Timestamp,
    pub last_loss_ts: Timestamp,
    pub consecutive_errors: u32,
    pub consecutive_losses: u32,
}

impl RiskState {
    /// Check if in cooldown after loss
    pub fn in_cooldown(&self, now: Timestamp, cooldown_ms: u64) -> bool {
        now.saturating_sub(self.last_loss_ts) < cooldown_ms
    }

    /// Reset daily counters
    pub fn reset_day(&mut self, day: u64) {
        if self.trade_day != day {
            self.trade_day = day;
            self.trades_today = 0;
            self.daily_pnl = 0.0;
        }
    }
}

/// Per-strategy state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineStrategyState {
    pub id: String,
    pub enabled: bool,
    pub score: f64,
    pub last_signal_ts: Timestamp,
    pub custom: HashMap<String, f64>,
}
