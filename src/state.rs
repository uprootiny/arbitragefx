use std::collections::HashMap;

use crate::exchange::Candle as ExCandle;
use crate::strategy::{IndicatorSnapshot, MarketAux, MarketView, MetricsState, PortfolioState, Strategy, StrategyState};

#[derive(Clone)]
pub struct Config {
    pub symbol: String,
    pub candle_granularity: u64,
    pub window: usize,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub binance_base: String,
    pub binance_fapi_base: String,
    pub kraken_base: String,
    pub sqlite_path: String,
    pub persist_every_secs: u64,
    pub max_position_pct: f64,
    pub max_daily_loss_pct: f64,
    pub max_trades_per_day: u32,
    pub cooldown_secs: u64,
    pub vol_pause_mult: f64,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub breakout_threshold: f64,
    pub edge_hurdle: f64,
    pub edge_scale: f64,
    pub ema_fast: u32,
    pub ema_slow: u32,
    pub vol_window: u32,
    pub volume_window: u32,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub time_stop: u32,
    pub funding_high: f64,
    pub funding_spread: f64,
    pub liq_score_th: f64,
    pub depeg_th: f64,
    pub vol_low: f64,
    pub vol_high: f64,
    pub mom_th: f64,
    pub stretch_th: f64,
    pub kill_file: String,
    pub wal_path: String,
    pub reconcile_secs: u64,
    pub cancel_after_candles: u64,
    pub reconcile_drift_pct: f64,
    pub reconcile_drift_abs: f64,
    pub max_fill_slip_pct: f64,
    pub fill_channel_capacity: usize,
    pub allow_unknown_regime: bool,
    pub max_latency_ms: u64,
    pub max_liquidity_spread: f64,
    /// Minimum candles to hold a position before allowing exit (reduces overtrading)
    pub min_hold_candles: u32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            symbol: std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string()),
            candle_granularity: std::env::var("CANDLE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(300),
            window: std::env::var("WINDOW").ok().and_then(|v| v.parse().ok()).unwrap_or(500),
            api_key: std::env::var("API_KEY").ok(),
            api_secret: std::env::var("API_SECRET").ok(),
            binance_base: std::env::var("BINANCE_BASE").unwrap_or_else(|_| "https://api.binance.com".to_string()),
            binance_fapi_base: std::env::var("BINANCE_FAPI_BASE").unwrap_or_else(|_| "https://fapi.binance.com".to_string()),
            kraken_base: std::env::var("KRAKEN_BASE").unwrap_or_else(|_| "https://api.kraken.com".to_string()),
            sqlite_path: std::env::var("SQLITE_PATH").unwrap_or_else(|_| "./bot.sqlite".to_string()),
            persist_every_secs: std::env::var("PERSIST_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(300),
            max_position_pct: std::env::var("MAX_POS_PCT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05),
            max_daily_loss_pct: std::env::var("MAX_DAILY_LOSS_PCT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.02),
            max_trades_per_day: std::env::var("MAX_TRADES_DAY").ok().and_then(|v| v.parse().ok()).unwrap_or(20),
            cooldown_secs: std::env::var("COOLDOWN_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(600),
            vol_pause_mult: std::env::var("VOL_PAUSE_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(2.5),
            entry_threshold: std::env::var("ENTRY_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(1.2),
            exit_threshold: std::env::var("EXIT_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.4),
            breakout_threshold: std::env::var("BREAKOUT_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0),
            edge_hurdle: std::env::var("EDGE_HURDLE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003),
            edge_scale: std::env::var("EDGE_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0025),
            ema_fast: std::env::var("EMA_FAST").ok().and_then(|v| v.parse().ok()).unwrap_or(6),
            ema_slow: std::env::var("EMA_SLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(24),
            vol_window: std::env::var("VOL_WINDOW").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            volume_window: std::env::var("VOL_MEAN_WINDOW").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            take_profit: std::env::var("TAKE_PROFIT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.006),
            stop_loss: std::env::var("STOP_LOSS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.004),
            time_stop: std::env::var("TIME_STOP").ok().and_then(|v| v.parse().ok()).unwrap_or(12),
            funding_high: std::env::var("FUNDING_HIGH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0001),
            funding_spread: std::env::var("FUNDING_SPREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(0.00005),
            liq_score_th: std::env::var("LIQ_SCORE_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0),
            depeg_th: std::env::var("DEPEG_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.002),
            vol_low: std::env::var("VOL_LOW").ok().and_then(|v| v.parse().ok()).unwrap_or(0.6),
            vol_high: std::env::var("VOL_HIGH").ok().and_then(|v| v.parse().ok()).unwrap_or(1.6),
            mom_th: std::env::var("MOM_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.4),
            stretch_th: std::env::var("STRETCH_TH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.8),
            kill_file: std::env::var("KILL_FILE").unwrap_or_else(|_| "/tmp/STOP".to_string()),
            wal_path: std::env::var("WAL_PATH").unwrap_or_else(|_| "./bot.wal".to_string()),
            reconcile_secs: std::env::var("RECONCILE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            cancel_after_candles: std::env::var("CANCEL_AFTER_CANDLES").ok().and_then(|v| v.parse().ok()).unwrap_or(3),
            reconcile_drift_pct: std::env::var("RECONCILE_DRIFT_PCT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.02),
            reconcile_drift_abs: std::env::var("RECONCILE_DRIFT_ABS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0005),
            max_fill_slip_pct: std::env::var("MAX_FILL_SLIP_PCT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.02),
            fill_channel_capacity: std::env::var("FILL_CHANNEL_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(256),
            allow_unknown_regime: std::env::var("ALLOW_UNKNOWN_REGIME").map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes")).unwrap_or(false),
            max_latency_ms: std::env::var("MAX_LATENCY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(300000),
            max_liquidity_spread: std::env::var("MAX_LIQ_SPREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05),
            min_hold_candles: std::env::var("MIN_HOLD_CANDLES").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
        }
    }

    pub fn sleep_until_next_candle(&self, now_ts: u64) -> u64 {
        let next = ((now_ts / self.candle_granularity) + 1) * self.candle_granularity;
        next.saturating_sub(now_ts)
    }
}

pub fn now_ts() -> u64 {
    chrono::Utc::now().timestamp() as u64
}

#[derive(Debug, Clone, Copy)]
pub struct Fill {
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
    pub ts: u64,
}

#[derive(Clone)]
struct RingBuffer<T: Copy> {
    buf: Vec<T>,
    idx: usize,
    filled: bool,
}

impl<T: Copy> RingBuffer<T> {
    fn new(size: usize, zero: T) -> Self {
        Self { buf: vec![zero; size], idx: 0, filled: false }
    }

    fn push(&mut self, value: T) -> Option<T> {
        let old = if self.filled { Some(self.buf[self.idx]) } else { None };
        self.buf[self.idx] = value;
        self.idx = (self.idx + 1) % self.buf.len();
        if self.idx == 0 { self.filled = true; }
        old
    }

    fn last(&self) -> T {
        let i = if self.idx == 0 { self.buf.len() - 1 } else { self.idx - 1 };
        self.buf[i]
    }
}

#[derive(Clone)]
struct IndicatorState {
    ema_fast: f64,
    ema_slow: f64,
    price_n: u64,
    price_mean: f64,
    price_m2: f64,
    vol_mean: f64,
    vol_m2: f64,
    vol_n: u64,
    volume_mean: f64,
    volume_m2: f64,
    volume_n: u64,
    mom_mean: f64,
    mom_m2: f64,
    mom_n: u64,
    stretch_mean: f64,
    stretch_m2: f64,
    stretch_n: u64,
    vspike_mean: f64,
    vspike_m2: f64,
    vspike_n: u64,
    sum_px_vol: f64,
    sum_vol: f64,
    last_vol: f64,
    last_volume_spike: f64,
    last_stretch: f64,
}

impl IndicatorState {
    fn new() -> Self {
        Self {
            ema_fast: 0.0,
            ema_slow: 0.0,
            price_n: 0,
            price_mean: 0.0,
            price_m2: 0.0,
            vol_mean: 0.0,
            vol_m2: 0.0,
            vol_n: 0,
            volume_mean: 0.0,
            volume_m2: 0.0,
            volume_n: 0,
            mom_mean: 0.0,
            mom_m2: 0.0,
            mom_n: 0,
            stretch_mean: 0.0,
            stretch_m2: 0.0,
            stretch_n: 0,
            vspike_mean: 0.0,
            vspike_m2: 0.0,
            vspike_n: 0,
            sum_px_vol: 0.0,
            sum_vol: 0.0,
            last_vol: 0.0,
            last_volume_spike: 0.0,
            last_stretch: 0.0,
        }
    }

    fn update(&mut self, price: f64, volume: f64, alpha_fast: f64, alpha_slow: f64) {
        self.ema_fast = if self.ema_fast == 0.0 { price } else { alpha_fast * price + (1.0 - alpha_fast) * self.ema_fast };
        self.ema_slow = if self.ema_slow == 0.0 { price } else { alpha_slow * price + (1.0 - alpha_slow) * self.ema_slow };

        self.price_n += 1;
        let pdelta = price - self.price_mean;
        self.price_mean += pdelta / self.price_n as f64;
        let pdelta2 = price - self.price_mean;
        self.price_m2 += pdelta * pdelta2;

        self.volume_n += 1;
        let vdelta = volume - self.volume_mean;
        self.volume_mean += vdelta / self.volume_n as f64;
        let vdelta2 = volume - self.volume_mean;
        self.volume_m2 += vdelta * vdelta2;

        self.sum_px_vol += price * volume;
        self.sum_vol += volume;

        let var = if self.price_n > 1 { self.price_m2 / (self.price_n as f64 - 1.0) } else { 0.0 };
        let vol = var.sqrt();
        self.last_vol = vol;
        self.vol_n += 1;
        let voldelta = vol - self.vol_mean;
        self.vol_mean += voldelta / self.vol_n as f64;
        let voldelta2 = vol - self.vol_mean;
        self.vol_m2 += voldelta * voldelta2;

        let vwap = if self.sum_vol > 0.0 { self.sum_px_vol / self.sum_vol } else { price };
        let momentum = self.ema_fast - self.ema_slow;
        let stretch = if vwap > 0.0 { (price - vwap) / vwap } else { 0.0 };
        let vspike = if self.volume_mean > 0.0 { volume / self.volume_mean } else { 0.0 };
        self.last_stretch = stretch;
        self.last_volume_spike = vspike;

        self.mom_n += 1;
        let mdelta = momentum - self.mom_mean;
        self.mom_mean += mdelta / self.mom_n as f64;
        let mdelta2 = momentum - self.mom_mean;
        self.mom_m2 += mdelta * mdelta2;

        self.stretch_n += 1;
        let sdelta = stretch - self.stretch_mean;
        self.stretch_mean += sdelta / self.stretch_n as f64;
        let sdelta2 = stretch - self.stretch_mean;
        self.stretch_m2 += sdelta * sdelta2;

        self.vspike_n += 1;
        let vsdelta = vspike - self.vspike_mean;
        self.vspike_mean += vsdelta / self.vspike_n as f64;
        let vsdelta2 = vspike - self.vspike_mean;
        self.vspike_m2 += vsdelta * vsdelta2;
    }

    fn zscore(val: f64, mean: f64, m2: f64, n: u64) -> f64 {
        if n > 1 {
            let var = m2 / (n as f64 - 1.0);
            if var > 0.0 { (val - mean) / var.sqrt() } else { 0.0 }
        } else {
            0.0
        }
    }

    fn snapshot(&self) -> IndicatorSnapshot {
        let momentum = self.ema_fast - self.ema_slow;
        IndicatorSnapshot {
            ema_fast: self.ema_fast,
            ema_slow: self.ema_slow,
            vwap: if self.sum_vol > 0.0 { self.sum_px_vol / self.sum_vol } else { 0.0 },
            vol: self.last_vol,
            vol_mean: self.vol_mean,
            momentum,
            volume_spike: self.last_volume_spike,
            stretch: self.last_stretch,
            z_momentum: Self::zscore(momentum, self.mom_mean, self.mom_m2, self.mom_n),
            z_vol: Self::zscore(self.last_vol, self.vol_mean, self.vol_m2, self.vol_n),
            z_volume_spike: Self::zscore(self.last_volume_spike, self.vspike_mean, self.vspike_m2, self.vspike_n),
            z_stretch: Self::zscore(self.last_stretch, self.stretch_mean, self.stretch_m2, self.stretch_n),
        }
    }
}

pub struct MarketState {
    cfg: Config,
    buffers: HashMap<String, RingBuffer<ExCandle>>,
    indicators: HashMap<String, IndicatorState>,
    aux: HashMap<String, MarketAux>,
}

impl MarketState {
    pub fn new(cfg: Config) -> Self {
        Self { cfg, buffers: HashMap::new(), indicators: HashMap::new(), aux: HashMap::new() }
    }

    pub fn on_candle(&mut self, candle: ExCandle) {
        let sym = self.cfg.symbol.clone();
        let zero = ExCandle { ts: 0, o: 0.0, h: 0.0, l: 0.0, c: 0.0, v: 0.0 };
        let buf = self.buffers.entry(sym.clone()).or_insert_with(|| RingBuffer::new(self.cfg.window, zero));
        let _old = buf.push(candle);
        let ind = self.indicators.entry(sym).or_insert_with(IndicatorState::new);
        let alpha_fast = 2.0 / (self.cfg.ema_fast as f64 + 1.0);
        let alpha_slow = 2.0 / (self.cfg.ema_slow as f64 + 1.0);
        ind.update(candle.c, candle.v, alpha_fast, alpha_slow);
    }

    pub fn view<'a>(&self, symbol: &'a str) -> MarketView<'a> {
        let buf = self.buffers.get(symbol);
        let ind = self.indicators.get(symbol);
        let last = if let Some(buf) = buf {
            crate::strategy::Candle { ts: buf.last().ts, o: buf.last().o, h: buf.last().h, l: buf.last().l, c: buf.last().c, v: buf.last().v }
        } else {
            crate::strategy::Candle { ts: 0, o: 0.0, h: 0.0, l: 0.0, c: 0.0, v: 0.0 }
        };
        let indicators = ind.map(|i| i.snapshot()).unwrap_or_default();
        let aux = *self.aux.get(symbol).unwrap_or(&MarketAux::default());
        MarketView {
            symbol,
            last,
            indicators,
            aux,
        }
    }

    pub fn update_aux(&mut self, symbol: &str, aux: MarketAux) {
        self.aux.insert(symbol.to_string(), aux);
    }
}

pub struct StrategyInstance {
    pub id: String,
    pub strategy: Box<dyn Strategy + Send + Sync>,
    pub state: StrategyState,
}

impl StrategyInstance {
    pub fn build_default_set(cfg: Config) -> Vec<Self> {
        let mut list = Vec::new();
        for i in 0..3 {
            let offset = (i as u64) * 300;
            let id = format!("mom-{}", i);
            list.push(Self {
                id: id.clone(),
                strategy: Box::new(SimpleMomentum { id, start_delay: offset, cfg: cfg.clone() }),
                state: StrategyState {
                    portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
                    metrics: MetricsState::default(),
                    last_trade_ts: 0,
                    last_loss_ts: 0,
                    trading_halted: false,
                    trades_today: 0,
                    trade_day: 0,
                    order_seq: 0,
                },
            });
        }
        list
    }

    pub fn build_churn_set(cfg: Config) -> Vec<Self> {
        let mut list = Vec::new();
        let variants = [
            (0.8, 0.0020, 0.006, 0.004),
            (1.0, 0.0025, 0.006, 0.004),
            (1.2, 0.0025, 0.006, 0.004),
            (1.4, 0.0030, 0.006, 0.004),
            (0.8, 0.0020, 0.008, 0.006),
            (1.0, 0.0025, 0.008, 0.006),
            (1.2, 0.0025, 0.008, 0.006),
            (1.4, 0.0030, 0.008, 0.006),
            (0.9, 0.0022, 0.005, 0.003),
            (1.1, 0.0026, 0.005, 0.003),
            (1.3, 0.0032, 0.005, 0.003),
            (1.5, 0.0036, 0.005, 0.003),
        ];

        for (i, (entry_th, edge_scale, tp, sl)) in variants.iter().enumerate() {
            let mut cfg_i = cfg.clone();
            cfg_i.entry_threshold = *entry_th;
            cfg_i.edge_scale = *edge_scale;
            cfg_i.take_profit = *tp;
            cfg_i.stop_loss = *sl;
            let id = format!("churn-{}", i);
            list.push(Self {
                id: id.clone(),
                strategy: Box::new(SimpleMomentum {
                    id,
                    start_delay: (i as u64) * 300,
                    cfg: cfg_i,
                }),
                state: StrategyState {
                    portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
                    metrics: MetricsState::default(),
                    last_trade_ts: 0,
                    last_loss_ts: 0,
                    trading_halted: false,
                    trades_today: 0,
                    trade_day: 0,
                    order_seq: 0,
                },
            });
        }
        list
    }

    pub fn build_carry_event_set(cfg: Config) -> Vec<Self> {
        let mut list = Vec::new();
        for i in 0..3 {
            let id = format!("carry-{}", i);
            list.push(Self {
                id: id.clone(),
                strategy: Box::new(CarryOpportunistic { id, cfg: cfg.clone() }),
                state: StrategyState {
                    portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
                    metrics: MetricsState::default(),
                    last_trade_ts: 0,
                    last_loss_ts: 0,
                    trading_halted: false,
                    trades_today: 0,
                    trade_day: 0,
                    order_seq: 0,
                },
            });
        }
        list
    }
}

struct SimpleMomentum {
    #[allow(dead_code)]
    id: String,
    start_delay: u64,
    cfg: Config,
}

impl Strategy for SimpleMomentum {
    fn id(&self) -> &'static str {
        "simple-momentum"
    }

    fn aux_requirements(&self) -> crate::strategy::AuxRequirements {
        crate::strategy::AuxRequirements::full()
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> crate::strategy::Action {
        let now = market.last.ts;
        if now < self.start_delay {
            return crate::strategy::Action::Hold;
        }
        if market.indicators.vol_mean > 0.0 {
            let vol_ratio = market.indicators.vol / market.indicators.vol_mean;
            if vol_ratio > self.cfg.vol_pause_mult {
                return crate::strategy::Action::Hold;
            }
        }

        // Trend filter: detect regime from EMA crossover
        let in_uptrend = market.indicators.ema_fast > market.indicators.ema_slow;
        let in_downtrend = market.indicators.ema_fast < market.indicators.ema_slow;
        let trend_strength = if market.indicators.ema_slow > 0.0 {
            (market.indicators.ema_fast - market.indicators.ema_slow).abs() / market.indicators.ema_slow
        } else {
            0.0
        };
        let strong_trend = trend_strength > 0.01; // 1% divergence = strong trend

        // Balanced score: momentum-aligned, no mean-reversion bonus against trend
        let stretch_contrib = if (in_uptrend && market.indicators.z_stretch < 0.0)
            || (in_downtrend && market.indicators.z_stretch > 0.0)
        {
            // Mean reversion aligns with trend, allow it
            -0.4 * market.indicators.z_stretch
        } else {
            // Mean reversion against trend, ignore or penalize
            0.0
        };

        let score = 1.0 * market.indicators.z_momentum
            + 0.3 * market.indicators.z_vol
            + 0.5 * market.indicators.z_volume_spike
            + stretch_contrib;

        let expected_edge = score.abs() * self.cfg.edge_scale;
        if expected_edge < self.cfg.edge_hurdle {
            return crate::strategy::Action::Hold;
        }

        // Funding carry: prefer direction opposite funding pressure.
        if market.aux.funding_rate.abs() > self.cfg.funding_high
            && market.aux.borrow_rate < market.aux.funding_rate.abs() - self.cfg.funding_spread
        {
            if market.aux.funding_rate > 0.0 {
                return crate::strategy::Action::Sell { qty: 0.001 };
            } else {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
        }

        // Liquidation cascade: trade with impulse.
        if market.aux.has_liquidations && market.aux.liquidation_score > self.cfg.liq_score_th {
            if market.indicators.z_momentum > 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }

        // Stablecoin depeg snapback: if symbol is stable-quoted, fade depeg.
        if market.aux.has_depeg && market.aux.stable_depeg.abs() > self.cfg.depeg_th {
            if market.aux.stable_depeg < 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }

        if state.portfolio.position != 0.0 {
            let price = market.last.c;
            let entry = state.portfolio.entry_price.max(1e-9);
            let move_pct = (price - entry) / entry;
            let elapsed = now.saturating_sub(state.last_trade_ts);
            let min_hold_secs = self.cfg.min_hold_candles as u64 * self.cfg.candle_granularity;

            // Stop loss always fires regardless of min hold (capital preservation)
            if move_pct <= -self.cfg.stop_loss {
                return crate::strategy::Action::Close;
            }

            // Other exits respect min hold period to reduce overtrading
            if elapsed >= min_hold_secs {
                if move_pct >= self.cfg.take_profit {
                    return crate::strategy::Action::Close;
                }
                if elapsed >= self.cfg.time_stop as u64 * self.cfg.candle_granularity {
                    return crate::strategy::Action::Close;
                }
                if score.abs() < self.cfg.exit_threshold {
                    return crate::strategy::Action::Close;
                }
            }
            return crate::strategy::Action::Hold;
        }

        // Volatility regime switch: low vol => follow momentum; high vol => trend-aligned mean reversion.
        let vol_ratio = if market.indicators.vol_mean > 0.0 {
            market.indicators.vol / market.indicators.vol_mean
        } else {
            1.0
        };

        // Low volatility: follow momentum
        if vol_ratio < self.cfg.vol_low {
            if market.indicators.z_momentum > self.cfg.mom_th {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
            if market.indicators.z_momentum < -self.cfg.mom_th {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }
        // High volatility: only mean-revert if aligned with trend OR trend is weak
        else if vol_ratio > self.cfg.vol_high {
            // Stretched above in uptrend or weak trend: sell expecting reversion
            if market.indicators.z_stretch > self.cfg.stretch_th && (in_uptrend || !strong_trend) {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
            // Stretched below in downtrend or weak trend: buy expecting reversion
            if market.indicators.z_stretch < -self.cfg.stretch_th && (in_downtrend || !strong_trend) {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
            // In strong opposite trend, don't mean-revert - follow trend instead
            if strong_trend && in_downtrend && market.indicators.z_momentum < -self.cfg.mom_th {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
            if strong_trend && in_uptrend && market.indicators.z_momentum > self.cfg.mom_th {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
        }

        // Score-based entry with trend confirmation
        if score > self.cfg.entry_threshold && !in_downtrend {
            return crate::strategy::Action::Buy { qty: 0.001 };
        }
        if score < -self.cfg.entry_threshold && !in_uptrend {
            return crate::strategy::Action::Sell { qty: 0.001 };
        }
        // Strong trend override: follow momentum regardless of score
        if strong_trend && in_downtrend && market.indicators.z_momentum < -0.5 {
            return crate::strategy::Action::Sell { qty: 0.001 };
        }
        if strong_trend && in_uptrend && market.indicators.z_momentum > 0.5 {
            return crate::strategy::Action::Buy { qty: 0.001 };
        }
        crate::strategy::Action::Hold
    }
}

struct CarryOpportunistic {
    #[allow(dead_code)]
    id: String,
    cfg: Config,
}

impl Strategy for CarryOpportunistic {
    fn id(&self) -> &'static str {
        "carry-opportunistic"
    }

    fn aux_requirements(&self) -> crate::strategy::AuxRequirements {
        crate::strategy::AuxRequirements::full()
    }

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> crate::strategy::Action {
        // Funding carry: hold a small delta-hedged bias (modeled here as a single leg).
        if market.aux.funding_rate.abs() > self.cfg.funding_high
            && market.aux.borrow_rate < market.aux.funding_rate.abs() - self.cfg.funding_spread
        {
            if market.aux.funding_rate > 0.0 {
                return crate::strategy::Action::Sell { qty: 0.001 };
            } else {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
        }

        // Opportunistic bursts: liquidation cascade or stablecoin depeg.
        if market.aux.has_liquidations && market.aux.liquidation_score > self.cfg.liq_score_th {
            if market.indicators.z_momentum > 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }
        if market.aux.has_depeg && market.aux.stable_depeg.abs() > self.cfg.depeg_th {
            if market.aux.stable_depeg < 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }

        // Risk throttling: exit on vol spike or decaying score.
        if state.portfolio.position != 0.0 {
            let price = market.last.c;
            let entry = state.portfolio.entry_price.max(1e-9);
            let move_pct = (price - entry) / entry;

            // Stop loss always fires (capital preservation overrides hold period)
            if move_pct <= -self.cfg.stop_loss {
                return crate::strategy::Action::Close;
            }

            let elapsed = market.last.ts.saturating_sub(state.last_trade_ts);
            let min_hold_secs = self.cfg.min_hold_candles as u64 * self.cfg.candle_granularity;
            if elapsed >= min_hold_secs {
                let vol_ratio = if market.indicators.vol_mean > 0.0 {
                    market.indicators.vol / market.indicators.vol_mean
                } else {
                    1.0
                };
                if vol_ratio > self.cfg.vol_pause_mult {
                    return crate::strategy::Action::Close;
                }
                if move_pct >= self.cfg.take_profit {
                    return crate::strategy::Action::Close;
                }
            }
        }

        crate::strategy::Action::Hold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{Action, MarketAux};

    fn test_config() -> Config {
        Config {
            symbol: "BTCUSDT".to_string(),
            candle_granularity: 300,
            window: 100,
            api_key: None,
            api_secret: None,
            binance_base: String::new(),
            binance_fapi_base: String::new(),
            kraken_base: String::new(),
            sqlite_path: String::new(),
            persist_every_secs: 300,
            max_position_pct: 0.05,
            max_daily_loss_pct: 0.02,
            max_trades_per_day: 20,
            cooldown_secs: 600,
            vol_pause_mult: 2.5,
            entry_threshold: 1.2,
            exit_threshold: 0.4,
            breakout_threshold: 2.0,
            edge_hurdle: 0.003,
            edge_scale: 0.0025,
            ema_fast: 6,
            ema_slow: 24,
            vol_window: 30,
            volume_window: 30,
            take_profit: 0.006,
            stop_loss: 0.004,
            time_stop: 12,
            funding_high: 0.0001,
            funding_spread: 0.00005,
            liq_score_th: 3.0,
            depeg_th: 0.002,
            vol_low: 0.6,
            vol_high: 1.6,
            mom_th: 0.4,
            stretch_th: 0.8,
            kill_file: String::new(),
            wal_path: String::new(),
            reconcile_secs: 60,
            cancel_after_candles: 3,
            reconcile_drift_pct: 0.02,
            reconcile_drift_abs: 0.0005,
            max_fill_slip_pct: 0.02,
            fill_channel_capacity: 256,
            allow_unknown_regime: false,
            max_latency_ms: 300000,
            max_liquidity_spread: 0.01,
            min_hold_candles: 0,
        }
    }

    // ==========================================================================
    // Config tests
    // ==========================================================================

    #[test]
    fn test_sleep_until_next_candle_boundary() {
        let cfg = Config { candle_granularity: 300, ..test_config() };

        // Exactly at boundary
        assert_eq!(cfg.sleep_until_next_candle(300), 300);
        assert_eq!(cfg.sleep_until_next_candle(600), 300);

        // Just after boundary
        assert_eq!(cfg.sleep_until_next_candle(301), 299);
        assert_eq!(cfg.sleep_until_next_candle(599), 1);

        // Middle of candle
        assert_eq!(cfg.sleep_until_next_candle(450), 150);
    }

    #[test]
    fn test_sleep_until_next_candle_zero() {
        let cfg = Config { candle_granularity: 300, ..test_config() };
        assert_eq!(cfg.sleep_until_next_candle(0), 300);
    }

    // ==========================================================================
    // RingBuffer tests (via internal access)
    // ==========================================================================

    #[test]
    fn test_ring_buffer_basic() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3, 0);

        // Push first value
        assert!(buf.push(10).is_none());
        assert_eq!(buf.last(), 10);

        // Push more
        buf.push(20);
        buf.push(30);
        assert_eq!(buf.last(), 30);

        // Wrap around - should return old value
        let old = buf.push(40);
        assert_eq!(old, Some(10));
        assert_eq!(buf.last(), 40);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(2, 0);

        buf.push(1);
        buf.push(2);
        assert_eq!(buf.last(), 2);

        buf.push(3); // wraps
        assert_eq!(buf.last(), 3);

        buf.push(4); // wraps again
        assert_eq!(buf.last(), 4);
    }

    // ==========================================================================
    // IndicatorState tests
    // ==========================================================================

    #[test]
    fn test_indicator_ema_initialization() {
        let mut ind = IndicatorState::new();
        let alpha_fast = 2.0 / 7.0; // ema_fast=6
        let alpha_slow = 2.0 / 25.0; // ema_slow=24

        // First update: EMAs should equal price
        ind.update(100.0, 1000.0, alpha_fast, alpha_slow);
        assert_eq!(ind.ema_fast, 100.0);
        assert_eq!(ind.ema_slow, 100.0);
    }

    #[test]
    fn test_indicator_ema_convergence() {
        let mut ind = IndicatorState::new();
        let alpha_fast = 2.0 / 7.0;
        let alpha_slow = 2.0 / 25.0;

        // Initialize
        ind.update(100.0, 1000.0, alpha_fast, alpha_slow);

        // Price rises to 110 for several bars
        for _ in 0..50 {
            ind.update(110.0, 1000.0, alpha_fast, alpha_slow);
        }

        // Fast EMA should converge faster than slow
        assert!((ind.ema_fast - 110.0).abs() < 0.1);
        assert!((ind.ema_slow - 110.0).abs() < 0.5);
    }

    #[test]
    fn test_indicator_zscore_calculation() {
        let mut ind = IndicatorState::new();
        let alpha = 0.2;

        // Feed stable data
        for _ in 0..100 {
            ind.update(100.0, 1000.0, alpha, alpha);
        }

        let snapshot = ind.snapshot();
        // With stable data, z-scores should be near zero
        assert!(snapshot.z_momentum.abs() < 1.0);
        assert!(snapshot.z_vol.abs() < 1.0);
    }

    #[test]
    fn test_indicator_vwap_calculation() {
        let mut ind = IndicatorState::new();

        // Volume-weighted: 100*1000 + 110*2000 = 320000, total vol = 3000
        // VWAP = 320000/3000 = 106.67
        ind.update(100.0, 1000.0, 0.2, 0.1);
        ind.update(110.0, 2000.0, 0.2, 0.1);

        let vwap = ind.sum_px_vol / ind.sum_vol;
        assert!((vwap - 106.666).abs() < 0.01);
    }

    // ==========================================================================
    // MarketState tests
    // ==========================================================================

    #[test]
    fn test_market_state_on_candle() {
        let cfg = test_config();
        let mut market = MarketState::new(cfg.clone());

        let candle = ExCandle { ts: 1000, o: 99.0, h: 101.0, l: 98.0, c: 100.0, v: 5000.0 };
        market.on_candle(candle);

        let view = market.view(&cfg.symbol);
        assert_eq!(view.last.ts, 1000);
        assert_eq!(view.last.c, 100.0);
        assert_eq!(view.indicators.ema_fast, 100.0); // First candle initializes EMA
    }

    #[test]
    fn test_market_state_view_missing_symbol() {
        let cfg = test_config();
        let market = MarketState::new(cfg);

        // View for non-existent symbol should return defaults
        let view = market.view("NONEXISTENT");
        assert_eq!(view.last.ts, 0);
        assert_eq!(view.last.c, 0.0);
    }

    #[test]
    fn test_market_state_aux_update() {
        let cfg = test_config();
        let mut market = MarketState::new(cfg.clone());

        let aux = MarketAux {
            funding_rate: 0.0005,
            borrow_rate: 0.0001,
            liquidation_score: 2.0,
            stable_depeg: -0.001,
            fetch_ts: 1000,
            has_funding: true,
            has_borrow: true,
            has_liquidations: true,
            has_depeg: true,
        };
        market.update_aux(&cfg.symbol, aux);

        let view = market.view(&cfg.symbol);
        assert_eq!(view.aux.funding_rate, 0.0005);
        assert!(view.aux.has_funding);
    }

    // ==========================================================================
    // SimpleMomentum strategy tests
    // ==========================================================================

    #[test]
    fn test_simple_momentum_start_delay() {
        let cfg = test_config();
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 1000, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // Create a view with ts < start_delay
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 500, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot::default(),
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Hold), "Should hold before start_delay");
    }

    #[test]
    fn test_simple_momentum_vol_pause() {
        let mut cfg = test_config();
        cfg.vol_pause_mult = 2.0;
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // High volatility ratio triggers pause
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot { vol: 5.0, vol_mean: 2.0, ..Default::default() },
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Hold), "Should hold during vol spike");
    }

    #[test]
    fn test_simple_momentum_funding_carry_short() {
        let mut cfg = test_config();
        cfg.funding_high = 0.0001;
        cfg.funding_spread = 0.00005;
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // High positive funding + low borrow = short opportunity
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot {
                z_momentum: 2.0,
                z_vol: 1.0,
                z_volume_spike: 1.0,
                z_stretch: 0.0,
                vol: 1.0,
                vol_mean: 1.0,
                ..Default::default()
            },
            aux: MarketAux {
                funding_rate: 0.0005, // High positive funding
                borrow_rate: 0.0001,  // Low borrow
                has_funding: true,
                has_borrow: true,
                ..Default::default()
            },
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Sell { .. }), "Should short on high positive funding");
    }

    #[test]
    fn test_simple_momentum_liquidation_cascade() {
        let mut cfg = test_config();
        cfg.liq_score_th = 3.0;
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // High liquidation score + positive momentum = buy with cascade
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot {
                z_momentum: 1.5, // Positive momentum
                z_vol: 1.0,
                z_volume_spike: 1.0,
                z_stretch: 0.0,
                vol: 1.0,
                vol_mean: 1.0,
                ..Default::default()
            },
            aux: MarketAux {
                liquidation_score: 5.0, // Above threshold
                has_liquidations: true,
                ..Default::default()
            },
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Should buy on liq cascade with positive momentum");
    }

    #[test]
    fn test_simple_momentum_take_profit() {
        let mut cfg = test_config();
        cfg.take_profit = 0.006;
        cfg.edge_hurdle = 0.0; // Disable edge check for position exit test
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState {
                cash: 900.0,
                position: 0.1,
                entry_price: 100.0,
                equity: 1000.0,
            },
            metrics: MetricsState::default(),
            last_trade_ts: 500,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 1,
            trade_day: 0,
            order_seq: 1,
        };

        // Price moved up 1% (above take_profit 0.6%)
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 102.0, l: 100.0, c: 101.0, v: 1000.0 },
            indicators: IndicatorSnapshot { z_momentum: 2.0, vol: 1.0, vol_mean: 1.0, ..Default::default() },
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Close), "Should close on take profit");
    }

    #[test]
    fn test_simple_momentum_stop_loss() {
        let mut cfg = test_config();
        cfg.stop_loss = 0.004;
        cfg.edge_hurdle = 0.0; // Disable edge check for position exit test
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState {
                cash: 900.0,
                position: 0.1,
                entry_price: 100.0,
                equity: 1000.0,
            },
            metrics: MetricsState::default(),
            last_trade_ts: 500,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 1,
            trade_day: 0,
            order_seq: 1,
        };

        // Price moved down 0.5% (above stop_loss 0.4%)
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 100.0, l: 99.0, c: 99.5, v: 1000.0 },
            indicators: IndicatorSnapshot { z_momentum: 2.0, vol: 1.0, vol_mean: 1.0, ..Default::default() },
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Close), "Should close on stop loss");
    }

    #[test]
    fn test_simple_momentum_time_stop() {
        let mut cfg = test_config();
        cfg.time_stop = 12; // 12 candles
        cfg.candle_granularity = 300;
        cfg.edge_hurdle = 0.0; // Disable edge check for position exit test
        let mut strategy = SimpleMomentum { id: "test".to_string(), start_delay: 0, cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState {
                cash: 900.0,
                position: 0.1,
                entry_price: 100.0,
                equity: 1000.0,
            },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 1,
            trade_day: 0,
            order_seq: 1,
        };

        // 12 candles * 300 seconds = 3600 seconds elapsed
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 3601, o: 100.0, h: 100.5, l: 99.5, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot { z_momentum: 2.0, vol: 1.0, vol_mean: 1.0, ..Default::default() },
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Close), "Should close on time stop");
    }

    // ==========================================================================
    // CarryOpportunistic strategy tests
    // ==========================================================================

    #[test]
    fn test_carry_opportunistic_funding_long() {
        let mut cfg = test_config();
        cfg.funding_high = 0.0001;
        cfg.funding_spread = 0.00005;
        let mut strategy = CarryOpportunistic { id: "carry-test".to_string(), cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // High negative funding + low borrow = long opportunity
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot::default(),
            aux: MarketAux {
                funding_rate: -0.0005, // High negative funding
                borrow_rate: 0.0001,   // Low borrow
                has_funding: true,
                has_borrow: true,
                ..Default::default()
            },
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Should long on high negative funding");
    }

    #[test]
    fn test_carry_opportunistic_vol_exit() {
        let mut cfg = test_config();
        cfg.vol_pause_mult = 2.0;
        let mut strategy = CarryOpportunistic { id: "carry-test".to_string(), cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState {
                cash: 900.0,
                position: 0.1,
                entry_price: 100.0,
                equity: 1000.0,
            },
            metrics: MetricsState::default(),
            last_trade_ts: 500,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 1,
            trade_day: 0,
            order_seq: 1,
        };

        // Vol spike while in position = close
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot { vol: 5.0, vol_mean: 2.0, ..Default::default() },
            aux: MarketAux::default(),
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Close), "Should close on vol spike");
    }

    #[test]
    fn test_carry_depeg_snapback() {
        let mut cfg = test_config();
        cfg.depeg_th = 0.002;
        let mut strategy = CarryOpportunistic { id: "carry-test".to_string(), cfg: cfg.clone() };
        let mut state = StrategyState {
            portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
            metrics: MetricsState::default(),
            last_trade_ts: 0,
            last_loss_ts: 0,
            trading_halted: false,
            trades_today: 0,
            trade_day: 0,
            order_seq: 0,
        };

        // Negative depeg (stablecoin below peg) = buy expecting snapback
        let view = MarketView {
            symbol: "BTCUSDT",
            last: crate::strategy::Candle { ts: 1000, o: 100.0, h: 101.0, l: 99.0, c: 100.0, v: 1000.0 },
            indicators: IndicatorSnapshot::default(),
            aux: MarketAux {
                stable_depeg: -0.005, // Below peg
                has_depeg: true,
                ..Default::default()
            },
        };

        let action = strategy.update(view, &mut state);
        assert!(matches!(action, Action::Buy { .. }), "Should buy on negative depeg");
    }

    // ==========================================================================
    // StrategyInstance tests
    // ==========================================================================

    #[test]
    fn test_strategy_instance_default_set_count() {
        let cfg = test_config();
        let strategies = StrategyInstance::build_default_set(cfg);
        assert_eq!(strategies.len(), 3, "Default set should have 3 strategies");
    }

    #[test]
    fn test_strategy_instance_churn_set_count() {
        let cfg = test_config();
        let strategies = StrategyInstance::build_churn_set(cfg);
        assert_eq!(strategies.len(), 12, "Churn set should have 12 variants");
    }

    #[test]
    fn test_strategy_instance_carry_set_count() {
        let cfg = test_config();
        let strategies = StrategyInstance::build_carry_event_set(cfg);
        assert_eq!(strategies.len(), 3, "Carry set should have 3 strategies");
    }

    #[test]
    fn test_strategy_instance_unique_ids() {
        let cfg = test_config();
        let strategies = StrategyInstance::build_churn_set(cfg);
        let ids: Vec<_> = strategies.iter().map(|s| &s.id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "All strategy IDs should be unique");
    }

    #[test]
    fn test_strategy_instance_initial_equity() {
        let cfg = test_config();
        let strategies = StrategyInstance::build_default_set(cfg);
        for s in &strategies {
            assert_eq!(s.state.portfolio.equity, 1000.0);
            assert_eq!(s.state.portfolio.cash, 1000.0);
            assert_eq!(s.state.portfolio.position, 0.0);
        }
    }

    fn make_view(ts: u64, price: f64, indicators: IndicatorSnapshot, aux: MarketAux) -> MarketView<'static> {
        let candle = crate::strategy::Candle {
            ts,
            o: price,
            h: price,
            l: price,
            c: price,
            v: 1000.0,
        };
        MarketView {
            symbol: "BTCUSDT",
            last: candle,
            indicators,
            aux,
        }
    }

    fn default_state() -> StrategyState {
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
    fn test_simple_momentum_funding_carry_signal() {
        let cfg = test_config();
        let mut strat = SimpleMomentum { id: "mom".to_string(), start_delay: 0, cfg };
        let mut state = default_state();
        let indicators = IndicatorSnapshot {
            z_momentum: 2.0,
            ..IndicatorSnapshot::default()
        };
        let aux = MarketAux {
            funding_rate: 0.001,
            borrow_rate: 0.0,
            liquidation_score: 0.0,
            stable_depeg: 0.0,
            fetch_ts: 1000,
            has_funding: true,
            has_borrow: true,
            has_liquidations: false,
            has_depeg: false,
        };
        let view = make_view(1000, 30000.0, indicators, aux);
        let action = strat.update(view, &mut state);
        assert!(matches!(action, Action::Sell { .. }));
    }

    #[test]
    fn test_carry_opportunistic_depeg_signal() {
        let cfg = test_config();
        let mut strat = CarryOpportunistic { id: "carry".to_string(), cfg };
        let mut state = default_state();
        let indicators = IndicatorSnapshot::default();
        let aux = MarketAux {
            funding_rate: 0.0,
            borrow_rate: 0.0,
            liquidation_score: 0.0,
            stable_depeg: -0.01,
            fetch_ts: 1000,
            has_funding: false,
            has_borrow: false,
            has_liquidations: false,
            has_depeg: true,
        };
        let view = make_view(1000, 30000.0, indicators, aux);
        let action = strat.update(view, &mut state);
        assert!(matches!(action, Action::Buy { .. }));
    }
}
