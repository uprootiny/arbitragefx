#[derive(Debug, Clone)]
pub enum FeedEvent {
    FundingImbalance { symbol: String, rate: f64 },
    LiquidationWave { symbol: String, score: f64 },
    Depeg { symbol: String, delta: f64 },
    VolRegime { symbol: String, ratio: f64 },
}
