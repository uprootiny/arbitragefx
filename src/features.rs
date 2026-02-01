use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RollingStats {
    window: usize,
    buf: VecDeque<f64>,
    sum: f64,
    sumsq: f64,
}

impl RollingStats {
    pub fn new(window: usize) -> Self {
        Self { window, buf: VecDeque::new(), sum: 0.0, sumsq: 0.0 }
    }

    pub fn push(&mut self, x: f64) {
        self.buf.push_back(x);
        self.sum += x;
        self.sumsq += x * x;
        if self.buf.len() > self.window {
            if let Some(old) = self.buf.pop_front() {
                self.sum -= old;
                self.sumsq -= old * old;
            }
        }
    }

    pub fn mean(&self) -> f64 {
        if self.buf.is_empty() { 0.0 } else { self.sum / self.buf.len() as f64 }
    }

    pub fn variance(&self) -> f64 {
        let n = self.buf.len() as f64;
        if n < 2.0 { 0.0 } else { (self.sumsq - (self.sum * self.sum) / n) / (n - 1.0) }
    }

    pub fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.buf.is_empty() {
            return 0.0;
        }
        let mut v: Vec<f64> = self.buf.iter().cloned().collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((v.len() - 1) as f64 * p.clamp(0.0, 1.0)).round() as usize;
        v[idx]
    }
}

#[derive(Debug, Clone)]
pub struct FeatureSnapshot {
    pub funding_rate: f64,
    pub funding_p95: f64,
    pub funding_flip: bool,
    pub oi: f64,
    pub oi_change: f64,
    pub price_velocity: f64,
    pub vol_ratio: f64,
    pub vol_compress: bool,
    pub liquidation_score: f64,
    pub stable_depeg: f64,
}

pub struct FeaturePipeline {
    funding: RollingStats,
    oi: RollingStats,
    price: RollingStats,
    vol_short: RollingStats,
    vol_long: RollingStats,
    last_funding: f64,
    last_oi: f64,
    last_price: f64,
}

impl FeaturePipeline {
    pub fn new(funding_window: usize, oi_window: usize, vol_short: usize, vol_long: usize) -> Self {
        Self {
            funding: RollingStats::new(funding_window),
            oi: RollingStats::new(oi_window),
            price: RollingStats::new(vol_short),
            vol_short: RollingStats::new(vol_short),
            vol_long: RollingStats::new(vol_long),
            last_funding: 0.0,
            last_oi: 0.0,
            last_price: 0.0,
        }
    }

    pub fn update(
        &mut self,
        price: f64,
        funding_rate: f64,
        oi: f64,
        liq_score: f64,
        stable_depeg: f64,
    ) -> FeatureSnapshot {
        self.funding.push(funding_rate.abs());
        self.oi.push(oi);
        self.price.push(price);

        let price_vel = if self.last_price > 0.0 { (price - self.last_price) / self.last_price } else { 0.0 };
        self.last_price = price;

        self.vol_short.push(price_vel);
        self.vol_long.push(price_vel);
        let vol_ratio = if self.vol_long.mean().abs() > 0.0 {
            self.vol_short.stddev() / self.vol_long.stddev().max(1e-9)
        } else {
            1.0
        };

        let funding_flip = (self.last_funding >= 0.0 && funding_rate < 0.0)
            || (self.last_funding <= 0.0 && funding_rate > 0.0);
        self.last_funding = funding_rate;

        let oi_change = if self.last_oi > 0.0 { (oi - self.last_oi) / self.last_oi } else { 0.0 };
        self.last_oi = oi;

        let funding_p95 = self.funding.percentile(0.95);
        let vol_compress = self.vol_short.stddev() < self.vol_short.percentile(0.1);

        FeatureSnapshot {
            funding_rate,
            funding_p95,
            funding_flip,
            oi,
            oi_change,
            price_velocity: price_vel,
            vol_ratio,
            vol_compress,
            liquidation_score: liq_score,
            stable_depeg,
        }
    }
}
