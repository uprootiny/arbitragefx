//! Technical indicators - stateful computations over price series.
//!
//! Each indicator maintains its own state and can be updated incrementally.

use std::collections::VecDeque;

// =============================================================================
// Rolling Statistics
// =============================================================================

/// Exponential moving average with configurable alpha
#[derive(Debug, Clone)]
pub struct Ema {
    pub value: f64,
    pub alpha: f64,
    initialized: bool,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        Self {
            value: 0.0,
            alpha: 2.0 / (period as f64 + 1.0),
            initialized: false,
        }
    }

    pub fn with_alpha(alpha: f64) -> Self {
        Self { value: 0.0, alpha, initialized: false }
    }

    pub fn update(&mut self, price: f64) -> f64 {
        if !self.initialized {
            self.value = price;
            self.initialized = true;
        } else {
            self.value = self.value * (1.0 - self.alpha) + price * self.alpha;
        }
        self.value
    }

    pub fn get(&self) -> f64 {
        self.value
    }
}

/// Simple moving average with fixed window
#[derive(Debug, Clone)]
pub struct Sma {
    window: VecDeque<f64>,
    period: usize,
    sum: f64,
}

impl Sma {
    pub fn new(period: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(period),
            period,
            sum: 0.0,
        }
    }

    pub fn update(&mut self, price: f64) -> f64 {
        self.sum += price;
        self.window.push_back(price);
        if self.window.len() > self.period {
            self.sum -= self.window.pop_front().unwrap_or(0.0);
        }
        self.get()
    }

    pub fn get(&self) -> f64 {
        if self.window.is_empty() {
            0.0
        } else {
            self.sum / self.window.len() as f64
        }
    }

    pub fn is_ready(&self) -> bool {
        self.window.len() >= self.period
    }
}

/// Rolling standard deviation using Welford's algorithm
#[derive(Debug, Clone)]
pub struct RollingStd {
    window: VecDeque<f64>,
    period: usize,
    mean: f64,
    m2: f64,
}

impl RollingStd {
    pub fn new(period: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(period),
            period,
            mean: 0.0,
            m2: 0.0,
        }
    }

    pub fn update(&mut self, value: f64) -> f64 {
        self.window.push_back(value);

        // Recalculate from window (simple approach for correctness)
        if self.window.len() > self.period {
            self.window.pop_front();
        }

        let n = self.window.len() as f64;
        if n < 2.0 {
            return 0.0;
        }

        self.mean = self.window.iter().sum::<f64>() / n;
        self.m2 = self.window.iter().map(|x| (x - self.mean).powi(2)).sum::<f64>();

        (self.m2 / (n - 1.0)).sqrt()
    }

    pub fn get(&self) -> f64 {
        let n = self.window.len() as f64;
        if n < 2.0 {
            return 0.0;
        }
        (self.m2 / (n - 1.0)).sqrt()
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }
}

// =============================================================================
// Classic Technical Indicators
// =============================================================================

/// Relative Strength Index (RSI)
#[derive(Debug, Clone)]
pub struct Rsi {
    period: usize,
    avg_gain: f64,
    avg_loss: f64,
    prev_price: Option<f64>,
    count: usize,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            avg_gain: 0.0,
            avg_loss: 0.0,
            prev_price: None,
            count: 0,
        }
    }

    pub fn update(&mut self, price: f64) -> f64 {
        if let Some(prev) = self.prev_price {
            let change = price - prev;
            let gain = if change > 0.0 { change } else { 0.0 };
            let loss = if change < 0.0 { -change } else { 0.0 };

            self.count += 1;

            if self.count <= self.period {
                // Initial SMA period
                self.avg_gain = (self.avg_gain * (self.count - 1) as f64 + gain) / self.count as f64;
                self.avg_loss = (self.avg_loss * (self.count - 1) as f64 + loss) / self.count as f64;
            } else {
                // Smoothed average (Wilder's smoothing)
                let alpha = 1.0 / self.period as f64;
                self.avg_gain = self.avg_gain * (1.0 - alpha) + gain * alpha;
                self.avg_loss = self.avg_loss * (1.0 - alpha) + loss * alpha;
            }
        }
        self.prev_price = Some(price);
        self.get()
    }

    pub fn get(&self) -> f64 {
        if self.avg_loss == 0.0 {
            if self.avg_gain == 0.0 {
                50.0
            } else {
                100.0
            }
        } else {
            let rs = self.avg_gain / self.avg_loss;
            100.0 - (100.0 / (1.0 + rs))
        }
    }

    pub fn is_ready(&self) -> bool {
        self.count >= self.period
    }
}

/// MACD (Moving Average Convergence Divergence)
#[derive(Debug, Clone)]
pub struct Macd {
    fast_ema: Ema,
    slow_ema: Ema,
    signal_ema: Ema,
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
}

impl Macd {
    pub fn new(fast: usize, slow: usize, signal: usize) -> Self {
        Self {
            fast_ema: Ema::new(fast),
            slow_ema: Ema::new(slow),
            signal_ema: Ema::new(signal),
            macd_line: 0.0,
            signal_line: 0.0,
            histogram: 0.0,
        }
    }

    pub fn default_12_26_9() -> Self {
        Self::new(12, 26, 9)
    }

    pub fn update(&mut self, price: f64) {
        let fast = self.fast_ema.update(price);
        let slow = self.slow_ema.update(price);
        self.macd_line = fast - slow;
        self.signal_line = self.signal_ema.update(self.macd_line);
        self.histogram = self.macd_line - self.signal_line;
    }

    /// Returns (macd_line, signal_line, histogram)
    pub fn get(&self) -> (f64, f64, f64) {
        (self.macd_line, self.signal_line, self.histogram)
    }
}

/// Bollinger Bands
#[derive(Debug, Clone)]
pub struct BollingerBands {
    sma: Sma,
    std: RollingStd,
    multiplier: f64,
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
}

impl BollingerBands {
    pub fn new(period: usize, multiplier: f64) -> Self {
        Self {
            sma: Sma::new(period),
            std: RollingStd::new(period),
            multiplier,
            middle: 0.0,
            upper: 0.0,
            lower: 0.0,
        }
    }

    pub fn default_20_2() -> Self {
        Self::new(20, 2.0)
    }

    pub fn update(&mut self, price: f64) {
        self.middle = self.sma.update(price);
        let std = self.std.update(price);
        let band_width = std * self.multiplier;
        self.upper = self.middle + band_width;
        self.lower = self.middle - band_width;
    }

    pub fn percent_b(&self, price: f64) -> f64 {
        if self.upper == self.lower {
            0.5
        } else {
            (price - self.lower) / (self.upper - self.lower)
        }
    }

    pub fn bandwidth(&self) -> f64 {
        if self.middle == 0.0 {
            0.0
        } else {
            (self.upper - self.lower) / self.middle
        }
    }
}

/// Average True Range (ATR)
#[derive(Debug, Clone)]
pub struct Atr {
    period: usize,
    ema: Ema,
    prev_close: Option<f64>,
    pub value: f64,
}

impl Atr {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            ema: Ema::new(period),
            prev_close: None,
            value: 0.0,
        }
    }

    pub fn update(&mut self, high: f64, low: f64, close: f64) -> f64 {
        let tr = if let Some(prev_c) = self.prev_close {
            let hl = high - low;
            let hc = (high - prev_c).abs();
            let lc = (low - prev_c).abs();
            hl.max(hc).max(lc)
        } else {
            high - low
        };
        self.prev_close = Some(close);
        self.value = self.ema.update(tr);
        self.value
    }

    pub fn get(&self) -> f64 {
        self.value
    }
}

/// Stochastic Oscillator
#[derive(Debug, Clone)]
pub struct Stochastic {
    period: usize,
    highs: VecDeque<f64>,
    lows: VecDeque<f64>,
    k_smooth: Sma,
    pub k: f64,
    pub d: f64,
}

impl Stochastic {
    pub fn new(period: usize, _k_smooth: usize, d_smooth: usize) -> Self {
        Self {
            period,
            highs: VecDeque::with_capacity(period),
            lows: VecDeque::with_capacity(period),
            k_smooth: Sma::new(d_smooth),
            k: 50.0,
            d: 50.0,
        }
    }

    pub fn default_14_3_3() -> Self {
        Self::new(14, 3, 3)
    }

    pub fn update(&mut self, high: f64, low: f64, close: f64) {
        self.highs.push_back(high);
        self.lows.push_back(low);
        if self.highs.len() > self.period {
            self.highs.pop_front();
            self.lows.pop_front();
        }

        let highest = self.highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let lowest = self.lows.iter().cloned().fold(f64::INFINITY, f64::min);

        self.k = if highest == lowest {
            50.0
        } else {
            100.0 * (close - lowest) / (highest - lowest)
        };

        self.d = self.k_smooth.update(self.k);
    }
}

// =============================================================================
// Candle Pattern Detection
// =============================================================================

/// Candle data for pattern detection
#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

impl Candle {
    pub fn body(&self) -> f64 {
        (self.close - self.open).abs()
    }

    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    pub fn upper_wick(&self) -> f64 {
        self.high - self.close.max(self.open)
    }

    pub fn lower_wick(&self) -> f64 {
        self.close.min(self.open) - self.low
    }

    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    pub fn body_ratio(&self) -> f64 {
        if self.range() == 0.0 {
            0.0
        } else {
            self.body() / self.range()
        }
    }
}

/// Detects common candle patterns
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CandlePattern {
    None,
    Doji,
    Hammer,
    InvertedHammer,
    BullishEngulfing,
    BearishEngulfing,
    MorningStar,
    EveningStar,
    ThreeWhiteSoldiers,
    ThreeBlackCrows,
    Harami,
}

pub struct PatternDetector {
    candles: VecDeque<Candle>,
    atr: Atr,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            candles: VecDeque::with_capacity(5),
            atr: Atr::new(14),
        }
    }

    pub fn update(&mut self, candle: Candle) -> Vec<CandlePattern> {
        self.atr.update(candle.high, candle.low, candle.close);
        self.candles.push_back(candle);
        if self.candles.len() > 5 {
            self.candles.pop_front();
        }

        let mut patterns = Vec::new();
        let atr = self.atr.get().max(0.0001);

        // Single candle patterns
        if let Some(c) = self.candles.back() {
            // Doji: tiny body
            if c.body() < atr * 0.1 {
                patterns.push(CandlePattern::Doji);
            }

            // Hammer: small body at top, long lower wick
            if c.body_ratio() < 0.3 && c.lower_wick() > c.body() * 2.0 && c.upper_wick() < c.body() {
                patterns.push(CandlePattern::Hammer);
            }

            // Inverted hammer: small body at bottom, long upper wick
            if c.body_ratio() < 0.3 && c.upper_wick() > c.body() * 2.0 && c.lower_wick() < c.body() {
                patterns.push(CandlePattern::InvertedHammer);
            }
        }

        // Two candle patterns
        if self.candles.len() >= 2 {
            let prev = self.candles[self.candles.len() - 2];
            let curr = *self.candles.back().unwrap();

            // Bullish engulfing
            if prev.is_bearish() && curr.is_bullish()
                && curr.open < prev.close && curr.close > prev.open
            {
                patterns.push(CandlePattern::BullishEngulfing);
            }

            // Bearish engulfing
            if prev.is_bullish() && curr.is_bearish()
                && curr.open > prev.close && curr.close < prev.open
            {
                patterns.push(CandlePattern::BearishEngulfing);
            }

            // Harami (inside bar)
            if curr.high < prev.high && curr.low > prev.low {
                patterns.push(CandlePattern::Harami);
            }
        }

        // Three candle patterns
        if self.candles.len() >= 3 {
            let c1 = self.candles[self.candles.len() - 3];
            let c2 = self.candles[self.candles.len() - 2];
            let c3 = *self.candles.back().unwrap();

            // Three white soldiers
            if c1.is_bullish() && c2.is_bullish() && c3.is_bullish()
                && c2.close > c1.close && c3.close > c2.close
                && c2.open > c1.open && c3.open > c2.open
            {
                patterns.push(CandlePattern::ThreeWhiteSoldiers);
            }

            // Three black crows
            if c1.is_bearish() && c2.is_bearish() && c3.is_bearish()
                && c2.close < c1.close && c3.close < c2.close
                && c2.open < c1.open && c3.open < c2.open
            {
                patterns.push(CandlePattern::ThreeBlackCrows);
            }

            // Morning star
            if c1.is_bearish() && c1.body() > atr * 0.5
                && c2.body() < atr * 0.2  // small middle candle
                && c3.is_bullish() && c3.body() > atr * 0.5
                && c3.close > (c1.open + c1.close) / 2.0
            {
                patterns.push(CandlePattern::MorningStar);
            }

            // Evening star
            if c1.is_bullish() && c1.body() > atr * 0.5
                && c2.body() < atr * 0.2
                && c3.is_bearish() && c3.body() > atr * 0.5
                && c3.close < (c1.open + c1.close) / 2.0
            {
                patterns.push(CandlePattern::EveningStar);
            }
        }

        patterns
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Support/Resistance Detection
// =============================================================================

/// Detects support and resistance levels from price history
#[derive(Debug, Clone)]
pub struct SupportResistance {
    highs: VecDeque<f64>,
    lows: VecDeque<f64>,
    period: usize,
    tolerance: f64,
}

impl SupportResistance {
    pub fn new(period: usize, tolerance_pct: f64) -> Self {
        Self {
            highs: VecDeque::with_capacity(period),
            lows: VecDeque::with_capacity(period),
            period,
            tolerance: tolerance_pct,
        }
    }

    pub fn update(&mut self, high: f64, low: f64) {
        self.highs.push_back(high);
        self.lows.push_back(low);
        if self.highs.len() > self.period {
            self.highs.pop_front();
            self.lows.pop_front();
        }
    }

    /// Returns major support levels (local minima that were tested multiple times)
    pub fn support_levels(&self) -> Vec<f64> {
        self.find_levels(&self.lows, false)
    }

    /// Returns major resistance levels
    pub fn resistance_levels(&self) -> Vec<f64> {
        self.find_levels(&self.highs, true)
    }

    fn find_levels(&self, prices: &VecDeque<f64>, is_resistance: bool) -> Vec<f64> {
        if prices.len() < 5 {
            return Vec::new();
        }

        let mut levels = Vec::new();
        let prices: Vec<f64> = prices.iter().cloned().collect();

        // Find local extrema
        for i in 2..prices.len() - 2 {
            let is_extreme = if is_resistance {
                prices[i] > prices[i - 1] && prices[i] > prices[i - 2]
                    && prices[i] > prices[i + 1] && prices[i] > prices[i + 2]
            } else {
                prices[i] < prices[i - 1] && prices[i] < prices[i - 2]
                    && prices[i] < prices[i + 1] && prices[i] < prices[i + 2]
            };

            if is_extreme {
                // Check if near existing level
                let level = prices[i];
                let tolerance = level * self.tolerance;
                let near_existing = levels.iter().any(|&l: &f64| (l - level).abs() < tolerance);
                if !near_existing {
                    levels.push(level);
                }
            }
        }

        // Sort by importance (how many times tested)
        levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
        levels.truncate(5); // Keep top 5
        levels
    }

    /// Distance to nearest support (negative if below)
    pub fn distance_to_support(&self, price: f64) -> Option<f64> {
        self.support_levels()
            .into_iter()
            .filter(|&s| s < price)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|s| (price - s) / price)
    }

    /// Distance to nearest resistance (positive if below)
    pub fn distance_to_resistance(&self, price: f64) -> Option<f64> {
        self.resistance_levels()
            .into_iter()
            .filter(|&r| r > price)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|r| (r - price) / price)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ema() {
        let mut ema = Ema::new(10);
        for i in 1..=20 {
            ema.update(i as f64);
        }
        // EMA should be between first and last value, biased toward recent
        assert!(ema.get() > 10.0 && ema.get() < 20.0);
    }

    #[test]
    fn test_sma() {
        let mut sma = Sma::new(5);
        for i in 1..=5 {
            sma.update(i as f64);
        }
        assert_eq!(sma.get(), 3.0); // (1+2+3+4+5)/5 = 3
    }

    #[test]
    fn test_rsi_extremes() {
        let mut rsi = Rsi::new(14);
        // Consistent gains → RSI should be high
        let mut price = 100.0;
        for _ in 0..50 {
            price += 1.0;  // Each update is a gain
            rsi.update(price);
        }
        assert!(rsi.get() > 50.0, "RSI was {}", rsi.get());
    }

    #[test]
    fn test_bollinger_bands() {
        let mut bb = BollingerBands::default_20_2();
        for i in 1..=30 {
            bb.update(100.0 + (i % 5) as f64);
        }
        assert!(bb.upper > bb.middle);
        assert!(bb.middle > bb.lower);
    }

    #[test]
    fn test_candle_pattern_doji() {
        let mut detector = PatternDetector::new();
        // Initialize with some normal candles
        for _ in 0..5 {
            detector.update(Candle { open: 100.0, high: 102.0, low: 98.0, close: 101.0 });
        }
        // Doji candle
        let patterns = detector.update(Candle { open: 100.0, high: 102.0, low: 98.0, close: 100.01 });
        assert!(patterns.contains(&CandlePattern::Doji));
    }
}
