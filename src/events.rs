use crate::features::FeatureSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    FundingImbalance,
    LiquidationCascade,
    StablecoinDepeg,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub ts: u64,
    pub event: EventType,
    pub score: f64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct EventConfig {
    pub funding_pctl_mult: f64,
    pub oi_spike_th: f64,
    pub vel_mult: f64,
    pub liq_score_th: f64,
    pub depeg_th: f64,
}

impl EventConfig {
    pub fn from_env() -> Self {
        Self {
            funding_pctl_mult: std::env::var("FUNDING_PCTL_MULT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            oi_spike_th: std::env::var("OI_SPIKE_TH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.08),
            vel_mult: std::env::var("VEL_MULT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3.0),
            liq_score_th: std::env::var("LIQ_SCORE_TH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3.0),
            depeg_th: std::env::var("DEPEG_TH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.005),
        }
    }
}

pub fn detect_phase1(ts: u64, f: &FeatureSnapshot, cfg: &EventConfig) -> Vec<Event> {
    let mut out = Vec::new();

    // 1) Funding imbalance reversion: funding at/above p95 + OI spike.
    if f.funding_rate.abs() >= f.funding_p95 * cfg.funding_pctl_mult
        && f.oi_change > cfg.oi_spike_th
    {
        out.push(Event {
            ts,
            event: EventType::FundingImbalance,
            score: f.funding_rate.abs(),
            reason: "funding_p95_and_oi_spike",
        });
    }

    // 2) Liquidation cascade momentum: velocity spike + liq score.
    if f.price_velocity.abs() > cfg.vel_mult * f.vol_ratio && f.liquidation_score > cfg.liq_score_th
    {
        out.push(Event {
            ts,
            event: EventType::LiquidationCascade,
            score: f.liquidation_score,
            reason: "velocity_spike_and_liq",
        });
    }

    // 3) Stablecoin depeg snapback.
    if f.stable_depeg.abs() > cfg.depeg_th {
        out.push(Event {
            ts,
            event: EventType::StablecoinDepeg,
            score: f.stable_depeg.abs(),
            reason: "depeg_threshold",
        });
    }

    out
}
