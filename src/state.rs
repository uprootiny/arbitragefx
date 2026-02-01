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
    id: String,
    start_delay: u64,
    cfg: Config,
}

impl Strategy for SimpleMomentum {
    fn id(&self) -> &'static str {
        "simple-momentum"
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

        let score = 1.0 * market.indicators.z_momentum
            + 0.5 * market.indicators.z_vol
            + 0.7 * market.indicators.z_volume_spike
            - 0.8 * market.indicators.z_stretch;

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
        if market.aux.liquidation_score > self.cfg.liq_score_th {
            if market.indicators.z_momentum > 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }

        // Stablecoin depeg snapback: if symbol is stable-quoted, fade depeg.
        if market.aux.stable_depeg.abs() > self.cfg.depeg_th {
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
            if move_pct >= self.cfg.take_profit || move_pct <= -self.cfg.stop_loss {
                return crate::strategy::Action::Close;
            }
            let elapsed = now.saturating_sub(state.last_trade_ts);
            if elapsed >= self.cfg.time_stop as u64 * self.cfg.candle_granularity {
                return crate::strategy::Action::Close;
            }
            if score.abs() < self.cfg.exit_threshold {
                return crate::strategy::Action::Close;
            }
            return crate::strategy::Action::Hold;
        }

        // Volatility regime switch: low vol => follow momentum; high vol => mean reversion.
        let vol_ratio = if market.indicators.vol_mean > 0.0 {
            market.indicators.vol / market.indicators.vol_mean
        } else {
            1.0
        };
        if vol_ratio < self.cfg.vol_low {
            if market.indicators.z_momentum > self.cfg.mom_th {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
            if market.indicators.z_momentum < -self.cfg.mom_th {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        } else if vol_ratio > self.cfg.vol_high {
            if market.indicators.z_stretch > self.cfg.stretch_th {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
            if market.indicators.z_stretch < -self.cfg.stretch_th {
                return crate::strategy::Action::Buy { qty: 0.001 };
            }
        }

        if score > self.cfg.entry_threshold {
            return crate::strategy::Action::Buy { qty: 0.001 };
        }
        if score < -self.cfg.entry_threshold {
            return crate::strategy::Action::Sell { qty: 0.001 };
        }
        crate::strategy::Action::Hold
    }
}

struct CarryOpportunistic {
    id: String,
    cfg: Config,
}

impl Strategy for CarryOpportunistic {
    fn id(&self) -> &'static str {
        "carry-opportunistic"
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
        if market.aux.liquidation_score > self.cfg.liq_score_th {
            if market.indicators.z_momentum > 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }
        if market.aux.stable_depeg.abs() > self.cfg.depeg_th {
            if market.aux.stable_depeg < 0.0 {
                return crate::strategy::Action::Buy { qty: 0.001 };
            } else {
                return crate::strategy::Action::Sell { qty: 0.001 };
            }
        }

        // Risk throttling: exit on vol spike or decaying score.
        if state.portfolio.position != 0.0 {
            let vol_ratio = if market.indicators.vol_mean > 0.0 {
                market.indicators.vol / market.indicators.vol_mean
            } else {
                1.0
            };
            if vol_ratio > self.cfg.vol_pause_mult {
                return crate::strategy::Action::Close;
            }
            let price = market.last.c;
            let entry = state.portfolio.entry_price.max(1e-9);
            let move_pct = (price - entry) / entry;
            if move_pct >= self.cfg.take_profit || move_pct <= -self.cfg.stop_loss {
                return crate::strategy::Action::Close;
            }
        }

        crate::strategy::Action::Hold
    }
}
