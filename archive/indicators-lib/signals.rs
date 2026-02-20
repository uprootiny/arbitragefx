//! Signal building blocks for composable strategies.
//!
//! Each signal is a pure function: MarketView → SignalOutput
//! Signals can be combined with filters and sizing rules.

use crate::strategy::{IndicatorSnapshot, MarketAux};

/// Signal output with strength and direction
#[derive(Debug, Clone, Copy, Default)]
pub struct Signal {
    /// Direction: positive = bullish, negative = bearish, zero = neutral
    pub direction: f64,
    /// Strength/confidence (0.0 - 1.0)
    pub strength: f64,
    /// Name for logging
    pub source: &'static str,
}

impl Signal {
    pub fn neutral() -> Self {
        Self { direction: 0.0, strength: 0.0, source: "neutral" }
    }

    pub fn bullish(strength: f64, source: &'static str) -> Self {
        Self { direction: 1.0, strength: strength.clamp(0.0, 1.0), source }
    }

    pub fn bearish(strength: f64, source: &'static str) -> Self {
        Self { direction: -1.0, strength: strength.clamp(0.0, 1.0), source }
    }

    pub fn from_score(score: f64, source: &'static str) -> Self {
        Self {
            direction: score.signum(),
            strength: score.abs().min(3.0) / 3.0, // normalize to 0-1
            source,
        }
    }

    pub fn is_bullish(&self) -> bool {
        self.direction > 0.0 && self.strength >= 0.1
    }

    pub fn is_bearish(&self) -> bool {
        self.direction < 0.0 && self.strength >= 0.1
    }

    pub fn is_neutral(&self) -> bool {
        self.strength < 0.1
    }

    /// Combine with another signal (weighted average)
    pub fn combine(&self, other: &Signal, self_weight: f64) -> Signal {
        let other_weight = 1.0 - self_weight;
        let total_strength = self.strength * self_weight + other.strength * other_weight;
        let weighted_dir = (self.direction * self.strength * self_weight
            + other.direction * other.strength * other_weight)
            / total_strength.max(0.001);
        Signal {
            direction: weighted_dir,
            strength: total_strength,
            source: "combined",
        }
    }
}

// =============================================================================
// Momentum Signals
// =============================================================================

/// Pure momentum following: buy when momentum positive, sell when negative
pub fn momentum_signal(ind: &IndicatorSnapshot, threshold: f64) -> Signal {
    if ind.z_momentum > threshold {
        Signal::bullish((ind.z_momentum - threshold) / 2.0, "momentum")
    } else if ind.z_momentum < -threshold {
        Signal::bearish((ind.z_momentum.abs() - threshold) / 2.0, "momentum")
    } else {
        Signal::neutral()
    }
}

/// Rate of change momentum (velocity of momentum)
pub fn momentum_acceleration(ind: &IndicatorSnapshot, prev_momentum: f64) -> Signal {
    let accel = ind.z_momentum - prev_momentum;
    if accel.abs() < 0.1 {
        Signal::neutral()
    } else {
        Signal::from_score(accel * 2.0, "momentum_accel")
    }
}

// =============================================================================
// Mean Reversion Signals
// =============================================================================

/// Mean reversion: fade extreme stretches from moving average
pub fn mean_reversion_signal(ind: &IndicatorSnapshot, threshold: f64) -> Signal {
    if ind.z_stretch > threshold {
        // Price stretched above mean → expect reversion down
        Signal::bearish((ind.z_stretch - threshold) / 2.0, "mean_reversion")
    } else if ind.z_stretch < -threshold {
        // Price stretched below mean → expect reversion up
        Signal::bullish((ind.z_stretch.abs() - threshold) / 2.0, "mean_reversion")
    } else {
        Signal::neutral()
    }
}

/// Bollinger band mean reversion (using z-score of stretch)
pub fn bollinger_reversion(ind: &IndicatorSnapshot, bands: f64) -> Signal {
    // z_stretch already normalized; bands typically 2.0
    if ind.z_stretch > bands {
        Signal::bearish(0.5 + (ind.z_stretch - bands) * 0.25, "bollinger")
    } else if ind.z_stretch < -bands {
        Signal::bullish(0.5 + (ind.z_stretch.abs() - bands) * 0.25, "bollinger")
    } else {
        Signal::neutral()
    }
}

// =============================================================================
// Trend Signals
// =============================================================================

/// EMA crossover trend signal
pub fn trend_signal(ind: &IndicatorSnapshot) -> Signal {
    if ind.ema_slow <= 0.0 {
        return Signal::neutral();
    }
    let divergence = (ind.ema_fast - ind.ema_slow) / ind.ema_slow;
    if divergence > 0.005 {
        // Fast above slow by 0.5%+
        Signal::bullish(divergence.min(0.05) * 10.0, "trend_ema")
    } else if divergence < -0.005 {
        Signal::bearish(divergence.abs().min(0.05) * 10.0, "trend_ema")
    } else {
        Signal::neutral()
    }
}

/// Trend strength (how strong is current trend)
pub fn trend_strength(ind: &IndicatorSnapshot) -> f64 {
    if ind.ema_slow <= 0.0 {
        return 0.0;
    }
    ((ind.ema_fast - ind.ema_slow) / ind.ema_slow).abs()
}

/// Price vs VWAP (institutional flow proxy)
pub fn vwap_signal(ind: &IndicatorSnapshot, price: f64) -> Signal {
    if ind.vwap <= 0.0 {
        return Signal::neutral();
    }
    let deviation = (price - ind.vwap) / ind.vwap;
    if deviation > 0.003 {
        // Price above VWAP → bullish flow
        Signal::bullish(deviation.min(0.02) * 25.0, "vwap")
    } else if deviation < -0.003 {
        Signal::bearish(deviation.abs().min(0.02) * 25.0, "vwap")
    } else {
        Signal::neutral()
    }
}

// =============================================================================
// Volatility Signals
// =============================================================================

/// Volatility breakout: high vol can precede big moves
pub fn volatility_breakout(ind: &IndicatorSnapshot, threshold: f64) -> Signal {
    if ind.z_vol > threshold {
        // High volatility - direction from momentum
        let strength = (ind.z_vol - threshold) / 2.0;
        Signal::from_score(ind.z_momentum * strength, "vol_breakout")
    } else {
        Signal::neutral()
    }
}

/// Volatility regime (for filtering, not direction)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolRegime {
    Low,
    Normal,
    High,
    Extreme,
}

pub fn volatility_regime(ind: &IndicatorSnapshot) -> VolRegime {
    let vol_ratio = if ind.vol_mean > 0.0 { ind.vol / ind.vol_mean } else { 1.0 };
    if vol_ratio < 0.5 {
        VolRegime::Low
    } else if vol_ratio < 1.5 {
        VolRegime::Normal
    } else if vol_ratio < 3.0 {
        VolRegime::High
    } else {
        VolRegime::Extreme
    }
}

// =============================================================================
// Volume Signals
// =============================================================================

/// Volume spike signal: unusual volume often precedes continuation
pub fn volume_spike_signal(ind: &IndicatorSnapshot, threshold: f64) -> Signal {
    if ind.z_volume_spike > threshold {
        // High volume spike - trade with momentum
        let strength = (ind.z_volume_spike - threshold) / 2.0;
        if ind.z_momentum > 0.5 {
            Signal::bullish(strength, "volume_spike")
        } else if ind.z_momentum < -0.5 {
            Signal::bearish(strength, "volume_spike")
        } else {
            Signal::neutral()
        }
    } else {
        Signal::neutral()
    }
}

/// Climax detection: extreme volume + extreme price = potential reversal
pub fn climax_signal(ind: &IndicatorSnapshot) -> Signal {
    let vol_extreme = ind.z_volume_spike > 2.5;
    let price_extreme = ind.z_stretch.abs() > 2.0;

    if vol_extreme && price_extreme {
        // Climax reversal signal (fade the move)
        if ind.z_stretch > 0.0 {
            Signal::bearish(0.6, "climax")
        } else {
            Signal::bullish(0.6, "climax")
        }
    } else {
        Signal::neutral()
    }
}

// =============================================================================
// Funding/Carry Signals
// =============================================================================

/// Funding rate arbitrage: collect funding by being opposite crowd
pub fn funding_carry_signal(aux: &MarketAux, threshold: f64, spread_min: f64) -> Signal {
    if !aux.has_funding || !aux.has_borrow {
        return Signal::neutral();
    }

    let funding_abs = aux.funding_rate.abs();
    let net_carry = funding_abs - aux.borrow_rate;

    if funding_abs > threshold && net_carry > spread_min {
        // Profitable to collect funding
        if aux.funding_rate > 0.0 {
            // Longs pay shorts → go short
            Signal::bearish(net_carry * 100.0, "funding_carry")
        } else {
            // Shorts pay longs → go long
            Signal::bullish(net_carry * 100.0, "funding_carry")
        }
    } else {
        Signal::neutral()
    }
}

/// Funding extreme: very high funding can indicate reversal
pub fn funding_extreme_signal(aux: &MarketAux, extreme_threshold: f64) -> Signal {
    if !aux.has_funding {
        return Signal::neutral();
    }

    if aux.funding_rate > extreme_threshold {
        // Extremely bullish funding → crowded long → fade
        Signal::bearish(0.4, "funding_extreme")
    } else if aux.funding_rate < -extreme_threshold {
        // Extremely bearish funding → crowded short → fade
        Signal::bullish(0.4, "funding_extreme")
    } else {
        Signal::neutral()
    }
}

// =============================================================================
// Event Signals
// =============================================================================

/// Liquidation cascade: trade with the cascade momentum
pub fn liquidation_cascade_signal(aux: &MarketAux, ind: &IndicatorSnapshot, threshold: f64) -> Signal {
    if !aux.has_liquidations || aux.liquidation_score < threshold {
        return Signal::neutral();
    }

    // High liquidation activity - momentum likely to continue
    let strength = (aux.liquidation_score - threshold) / 2.0;
    if ind.z_momentum > 0.0 {
        Signal::bullish(strength.min(0.8), "liquidation")
    } else {
        Signal::bearish(strength.min(0.8), "liquidation")
    }
}

/// Stablecoin depeg: fade depegs expecting snap back
pub fn depeg_signal(aux: &MarketAux, threshold: f64) -> Signal {
    if !aux.has_depeg || aux.stable_depeg.abs() < threshold {
        return Signal::neutral();
    }

    // Depeg from $1 → expect reversion
    if aux.stable_depeg < -threshold {
        // Trading below peg → buy expecting return
        Signal::bullish((aux.stable_depeg.abs() - threshold) * 50.0, "depeg")
    } else if aux.stable_depeg > threshold {
        // Trading above peg → sell expecting return
        Signal::bearish((aux.stable_depeg - threshold) * 50.0, "depeg")
    } else {
        Signal::neutral()
    }
}

// =============================================================================
// Composite Signals
// =============================================================================

/// Trend-aligned momentum: only follow momentum in direction of trend
pub fn trend_aligned_momentum(ind: &IndicatorSnapshot, mom_th: f64) -> Signal {
    let trend = trend_signal(ind);
    let mom = momentum_signal(ind, mom_th);

    // Only signal when momentum aligns with trend
    if trend.is_bullish() && mom.is_bullish() {
        Signal::bullish(mom.strength * 0.8 + trend.strength * 0.2, "trend_momentum")
    } else if trend.is_bearish() && mom.is_bearish() {
        Signal::bearish(mom.strength * 0.8 + trend.strength * 0.2, "trend_momentum")
    } else {
        Signal::neutral()
    }
}

/// Trend-aligned mean reversion: only revert in direction of trend
pub fn trend_aligned_reversion(ind: &IndicatorSnapshot, stretch_th: f64) -> Signal {
    let trend = trend_signal(ind);
    let revert = mean_reversion_signal(ind, stretch_th);

    // Only revert when it aligns with trend
    if trend.is_bullish() && revert.is_bullish() {
        // Uptrend + stretched below = buy dip
        Signal::bullish(revert.strength, "trend_reversion")
    } else if trend.is_bearish() && revert.is_bearish() {
        // Downtrend + stretched above = sell rally
        Signal::bearish(revert.strength, "trend_reversion")
    } else {
        Signal::neutral()
    }
}

/// Multi-factor signal: combine multiple signals with weights
pub fn multi_factor(
    ind: &IndicatorSnapshot,
    aux: &MarketAux,
    weights: &SignalWeights,
) -> Signal {
    let mut total_dir = 0.0;
    let mut total_weight = 0.0;

    // Momentum
    if weights.momentum > 0.0 {
        let s = momentum_signal(ind, 0.5);
        total_dir += s.direction * s.strength * weights.momentum;
        total_weight += weights.momentum;
    }

    // Trend
    if weights.trend > 0.0 {
        let s = trend_signal(ind);
        total_dir += s.direction * s.strength * weights.trend;
        total_weight += weights.trend;
    }

    // Mean reversion
    if weights.reversion > 0.0 {
        let s = mean_reversion_signal(ind, 1.5);
        total_dir += s.direction * s.strength * weights.reversion;
        total_weight += weights.reversion;
    }

    // Volume
    if weights.volume > 0.0 {
        let s = volume_spike_signal(ind, 1.0);
        total_dir += s.direction * s.strength * weights.volume;
        total_weight += weights.volume;
    }

    // Funding
    if weights.funding > 0.0 {
        let s = funding_carry_signal(aux, 0.0003, 0.0001);
        total_dir += s.direction * s.strength * weights.funding;
        total_weight += weights.funding;
    }

    if total_weight < 0.01 {
        return Signal::neutral();
    }

    let final_dir = total_dir / total_weight;
    Signal::from_score(final_dir, "multi_factor")
}

/// Weights for multi-factor signal
#[derive(Debug, Clone, Copy)]
pub struct SignalWeights {
    pub momentum: f64,
    pub trend: f64,
    pub reversion: f64,
    pub volume: f64,
    pub funding: f64,
}

impl Default for SignalWeights {
    fn default() -> Self {
        Self {
            momentum: 1.0,
            trend: 0.5,
            reversion: 0.3,
            volume: 0.2,
            funding: 0.1,
        }
    }
}

impl SignalWeights {
    pub fn momentum_only() -> Self {
        Self { momentum: 1.0, trend: 0.0, reversion: 0.0, volume: 0.0, funding: 0.0 }
    }

    pub fn trend_following() -> Self {
        Self { momentum: 0.6, trend: 1.0, reversion: 0.0, volume: 0.3, funding: 0.0 }
    }

    pub fn mean_reversion() -> Self {
        Self { momentum: 0.0, trend: 0.3, reversion: 1.0, volume: 0.2, funding: 0.0 }
    }

    pub fn carry() -> Self {
        Self { momentum: 0.2, trend: 0.0, reversion: 0.0, volume: 0.0, funding: 1.0 }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ind(z_mom: f64, z_stretch: f64, z_vol: f64, ema_fast: f64, ema_slow: f64) -> IndicatorSnapshot {
        IndicatorSnapshot {
            ema_fast,
            ema_slow,
            z_momentum: z_mom,
            z_stretch,
            z_vol,
            vol: 100.0,
            vol_mean: 100.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_momentum_signal() {
        let ind = make_ind(2.0, 0.0, 1.0, 100.0, 100.0);
        let sig = momentum_signal(&ind, 0.5);
        assert!(sig.is_bullish());
        assert!(sig.strength > 0.5);
    }

    #[test]
    fn test_mean_reversion_signal() {
        // Stretched above → bearish (expect reversion down)
        let ind = make_ind(0.0, 2.5, 1.0, 100.0, 100.0);
        let sig = mean_reversion_signal(&ind, 1.5);
        assert!(sig.is_bearish());
    }

    #[test]
    fn test_trend_signal() {
        // Fast above slow → bullish
        let ind = make_ind(0.0, 0.0, 1.0, 102.0, 100.0);
        let sig = trend_signal(&ind);
        assert!(sig.is_bullish());
    }

    #[test]
    fn test_trend_aligned_momentum() {
        // Uptrend + positive momentum = strong bullish
        let ind = make_ind(1.5, 0.0, 1.0, 102.0, 100.0);
        let sig = trend_aligned_momentum(&ind, 0.5);
        assert!(sig.is_bullish());

        // Uptrend + negative momentum = neutral (conflicting)
        let ind2 = make_ind(-1.5, 0.0, 1.0, 102.0, 100.0);
        let sig2 = trend_aligned_momentum(&ind2, 0.5);
        assert!(sig2.is_neutral());
    }

    #[test]
    fn test_signal_combine() {
        let bull = Signal::bullish(0.8, "a");
        let bear = Signal::bearish(0.6, "b");
        let combined = bull.combine(&bear, 0.5);
        // Should be weakly bullish (0.8 > 0.6)
        assert!(combined.direction > 0.0);
        assert!(combined.strength > 0.0);
    }
}
