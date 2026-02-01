use crate::strategy::MarketView;

use super::events::FeedEvent;

pub fn scan(market: MarketView) -> Vec<FeedEvent> {
    let mut out = Vec::new();
    if market.aux.funding_rate.abs() > 0.0001 {
        out.push(FeedEvent::FundingImbalance {
            symbol: market.symbol.to_string(),
            rate: market.aux.funding_rate,
        });
    }
    if market.aux.liquidation_score > 3.0 {
        out.push(FeedEvent::LiquidationWave {
            symbol: market.symbol.to_string(),
            score: market.aux.liquidation_score,
        });
    }
    if market.aux.stable_depeg.abs() > 0.002 {
        out.push(FeedEvent::Depeg {
            symbol: market.symbol.to_string(),
            delta: market.aux.stable_depeg,
        });
    }
    if market.indicators.vol_mean > 0.0 {
        let ratio = market.indicators.vol / market.indicators.vol_mean;
        if ratio < 0.6 || ratio > 1.6 {
            out.push(FeedEvent::VolRegime {
                symbol: market.symbol.to_string(),
                ratio,
            });
        }
    }
    out
}
