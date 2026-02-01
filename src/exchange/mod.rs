use anyhow::Result;
use serde::Deserialize;
use async_trait::async_trait;

use crate::state::{Config, Fill};
use crate::strategy::{Action, MarketAux};

mod binance;
mod kraken;
pub mod signing;
pub mod retry;

#[derive(Clone, Copy, Debug)]
pub enum ExchangeKind {
    Binance,
    Kraken,
}

impl ExchangeKind {
    pub fn from_env() -> Self {
        match std::env::var("EXCHANGE").unwrap_or_else(|_| "binance".to_string()).as_str() {
            "kraken" => ExchangeKind::Kraken,
            _ => ExchangeKind::Binance,
        }
    }

    pub fn build(self, cfg: Config) -> Result<Box<dyn Exchange + Send + Sync>> {
        match self {
            ExchangeKind::Binance => Ok(Box::new(binance::Binance::new(cfg)?)),
            ExchangeKind::Kraken => Ok(Box::new(kraken::Kraken::new(cfg)?)),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Candle {
    pub ts: u64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

#[async_trait]
pub trait Exchange {
    async fn fetch_latest_candle(&self, symbol: &str, granularity: u64) -> Result<Candle>;
    async fn fetch_aux(&self, symbol: &str) -> Result<MarketAux>;
    async fn execute(&self, symbol: &str, action: Action, state: &crate::strategy::StrategyState) -> Result<Fill>;
}
