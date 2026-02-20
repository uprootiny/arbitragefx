use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::strategy::MarketAux;

/// Cached aux data with TTL and backoff
#[derive(Debug, Clone)]
struct CachedAux {
    data: MarketAux,
    fetched_at: Instant,
    fetch_failures: u32,
    last_failure: Option<Instant>,
}

impl CachedAux {
    fn new(data: MarketAux) -> Self {
        Self {
            data,
            fetched_at: Instant::now(),
            fetch_failures: 0,
            last_failure: None,
        }
    }

    fn is_fresh(&self, ttl_secs: u64) -> bool {
        self.fetched_at.elapsed() < Duration::from_secs(ttl_secs)
    }

    fn backoff_secs(&self) -> u64 {
        // Exponential backoff: 2^failures seconds, capped at 300s
        let base = 2u64.saturating_pow(self.fetch_failures.min(8));
        base.min(300)
    }

    fn can_retry(&self) -> bool {
        match self.last_failure {
            None => true,
            Some(last) => last.elapsed() >= Duration::from_secs(self.backoff_secs()),
        }
    }

    fn record_failure(&mut self) {
        self.fetch_failures = self.fetch_failures.saturating_add(1);
        self.last_failure = Some(Instant::now());
    }

    fn record_success(&mut self, data: MarketAux) {
        self.data = data;
        self.fetched_at = Instant::now();
        self.fetch_failures = 0;
        self.last_failure = None;
    }
}

/// Aggregates auxiliary market data from multiple sources
pub struct AuxDataFetcher {
    client: Client,
    liquidation_window: Arc<Mutex<LiquidationWindow>>,
    /// Per-symbol cache with TTL and backoff
    cache: Arc<Mutex<HashMap<String, CachedAux>>>,
    /// TTL for cached data in seconds
    cache_ttl_secs: u64,
}

/// Rolling window of recent liquidations for score calculation
struct LiquidationWindow {
    events: VecDeque<LiquidationEvent>,
    window_secs: u64,
}

#[allow(dead_code)]
struct LiquidationEvent {
    ts: Instant,
    size_usd: f64,
    side: String,
}

impl LiquidationWindow {
    fn new(window_secs: u64) -> Self {
        Self {
            events: VecDeque::new(),
            window_secs,
        }
    }

    fn add(&mut self, size_usd: f64, side: String) {
        self.events.push_back(LiquidationEvent {
            ts: Instant::now(),
            size_usd,
            side,
        });
        self.prune();
    }

    fn prune(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(self.window_secs);
        while let Some(front) = self.events.front() {
            if front.ts < cutoff {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate liquidation score: sum of recent liquidation volume weighted by recency
    fn score(&mut self) -> f64 {
        self.prune();
        let now = Instant::now();
        let window = self.window_secs as f64;

        self.events
            .iter()
            .map(|e| {
                let age = now.duration_since(e.ts).as_secs_f64();
                let weight = 1.0 - (age / window).min(1.0);
                e.size_usd * weight / 100_000.0 // normalize to reasonable scale
            })
            .sum()
    }
}

// Binance API response types
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BinanceFundingRate {
    symbol: String,
    #[serde(rename = "fundingRate")]
    funding_rate: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BinanceMarginAsset {
    asset: String,
    #[serde(rename = "borrowRate")]
    borrow_rate: Option<String>,
    #[serde(rename = "yearlyInterestRate")]
    yearly_interest_rate: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BinancePremiumIndex {
    symbol: String,
    #[serde(rename = "markPrice")]
    mark_price: String,
    #[serde(rename = "indexPrice")]
    index_price: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BinanceLiquidation {
    #[serde(rename = "o")]
    order: BinanceLiqOrder,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BinanceLiqOrder {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "q")]
    qty: String,
    #[serde(rename = "p")]
    price: String,
}

#[derive(Deserialize, Debug)]
struct CoinGeckoPrice {
    usd: f64,
}

#[derive(Deserialize, Debug)]
struct CoinGeckoPrices {
    tether: Option<CoinGeckoPrice>,
    #[serde(rename = "usd-coin")]
    usd_coin: Option<CoinGeckoPrice>,
}

impl AuxDataFetcher {
    pub fn new() -> Self {
        Self::with_ttl(60) // Default 60s TTL
    }

    pub fn with_ttl(cache_ttl_secs: u64) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            liquidation_window: Arc::new(Mutex::new(LiquidationWindow::new(300))), // 5 min window
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_ttl_secs,
        }
    }

    /// Fetch with caching and backoff
    pub async fn fetch(&self, symbol: &str) -> Result<MarketAux> {
        // Check cache first
        let should_fetch = {
            let cache = self
                .cache
                .lock()
                .map_err(|_| anyhow::anyhow!("aux cache lock poisoned"))?;
            match cache.get(symbol) {
                Some(cached) if cached.is_fresh(self.cache_ttl_secs) => {
                    return Ok(cached.data);
                }
                Some(cached) if !cached.can_retry() => {
                    return Ok(cached.data);
                }
                _ => true,
            }
        };

        if should_fetch {
            match self.fetch_fresh(symbol).await {
                Ok(data) => {
                    let mut cache = self
                        .cache
                        .lock()
                        .map_err(|_| anyhow::anyhow!("aux cache lock poisoned"))?;
                    cache
                        .entry(symbol.to_string())
                        .and_modify(|c| c.record_success(data))
                        .or_insert_with(|| CachedAux::new(data));
                    return Ok(data);
                }
                Err(e) => {
                    if let Ok(mut cache) = self.cache.lock() {
                        if let Some(cached) = cache.get_mut(symbol) {
                            cached.record_failure();
                            return Ok(cached.data);
                        }
                    }
                    return Err(e);
                }
            }
        }

        Err(anyhow::anyhow!("cache miss with no retry allowed"))
    }

    /// Fresh fetch without cache
    async fn fetch_fresh(&self, symbol: &str) -> Result<MarketAux> {
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Fetch all data concurrently
        let (funding, borrow, premium, depeg) = tokio::join!(
            self.fetch_funding_rate(symbol),
            self.fetch_borrow_rate(symbol),
            self.fetch_premium_index(symbol),
            self.fetch_stablecoin_depeg(),
        );

        // Track which fields have real data vs defaults
        let (funding_rate, has_funding) = match funding {
            Ok(v) => (v, true),
            Err(_) => (0.0, false),
        };
        let (borrow_rate, has_borrow) = match borrow {
            Ok(v) => (v, true),
            Err(_) => (0.0, false),
        };
        let premium_opt = premium.ok();
        let depeg_opt = depeg.ok();
        let has_depeg = premium_opt.is_some() || depeg_opt.is_some();

        // Calculate liquidation score from window
        let (liquidation_score, has_liquidations) = self
            .liquidation_window
            .lock()
            .map(|mut w| {
                let score = w.score();
                let has_events = !w.events.is_empty();
                (score, has_events)
            })
            .unwrap_or((0.0, false));

        // Use premium deviation as additional depeg signal for futures
        let premium_depeg = premium_opt.unwrap_or(0.0);
        let stable_depeg = depeg_opt.unwrap_or(0.0);

        // Combine depeg signals
        let combined_depeg = if stable_depeg.abs() > premium_depeg.abs() {
            stable_depeg
        } else {
            premium_depeg
        };

        Ok(MarketAux {
            funding_rate,
            borrow_rate,
            liquidation_score,
            stable_depeg: combined_depeg,
            fetch_ts: now_ts,
            has_funding,
            has_borrow,
            has_liquidations,
            has_depeg,
        })
    }

    /// Fetch funding rate from Binance Futures
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<f64> {
        let url = format!(
            "https://fapi.binance.com/fapi/v1/fundingRate?symbol={}&limit=1",
            symbol
        );

        let resp = self.client.get(&url).send().await?;
        let data: Vec<BinanceFundingRate> = resp.json().await?;

        data.first()
            .and_then(|f| f.funding_rate.parse::<f64>().ok())
            .ok_or_else(|| anyhow::anyhow!("no funding rate"))
    }

    /// Fetch borrow rate from Binance Margin API
    async fn fetch_borrow_rate(&self, symbol: &str) -> Result<f64> {
        // Extract base asset from pair (BTCUSDT -> BTC)
        let base_asset = symbol
            .strip_suffix("USDT")
            .or_else(|| symbol.strip_suffix("BUSD"))
            .or_else(|| symbol.strip_suffix("USD"))
            .unwrap_or(symbol);

        // Cross margin data endpoint (public)
        let url = "https://api.binance.com/sapi/v1/margin/crossMarginData";

        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("borrow rate unavailable"));
        }

        let data: Vec<BinanceMarginAsset> = resp.json().await?;

        data.iter()
            .find(|a| a.asset.eq_ignore_ascii_case(base_asset))
            .and_then(|a| {
                a.borrow_rate
                    .as_ref()
                    .and_then(|r| r.parse::<f64>().ok())
                    .or_else(|| {
                        a.yearly_interest_rate
                            .as_ref()
                            .and_then(|r| r.parse::<f64>().ok())
                            .map(|y| y / 365.0 / 24.0) // convert yearly to hourly
                    })
            })
            .ok_or_else(|| anyhow::anyhow!("no borrow rate for {}", base_asset))
    }

    /// Fetch premium index (mark - index price deviation)
    async fn fetch_premium_index(&self, symbol: &str) -> Result<f64> {
        let url = format!(
            "https://fapi.binance.com/fapi/v1/premiumIndex?symbol={}",
            symbol
        );

        let resp = self.client.get(&url).send().await?;
        let data: BinancePremiumIndex = resp.json().await?;

        let mark: f64 = data.mark_price.parse()?;
        let index: f64 = data.index_price.parse()?;

        if index > 0.0 {
            Ok((mark - index) / index)
        } else {
            Err(anyhow::anyhow!("invalid index price"))
        }
    }

    /// Fetch stablecoin prices from CoinGecko to detect depeg
    async fn fetch_stablecoin_depeg(&self) -> Result<f64> {
        let url =
            "https://api.coingecko.com/api/v3/simple/price?ids=tether,usd-coin&vs_currencies=usd";

        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("depeg data unavailable"));
        }

        let prices: CoinGeckoPrices = resp.json().await?;

        // Calculate max depeg from either USDT or USDC
        let usdt_depeg = prices.tether.map(|p| p.usd - 1.0);
        let usdc_depeg = prices.usd_coin.map(|p| p.usd - 1.0);

        match (usdt_depeg, usdc_depeg) {
            (None, None) => Err(anyhow::anyhow!("missing stablecoin prices")),
            (Some(v), None) | (None, Some(v)) => Ok(v),
            (Some(a), Some(b)) => {
                if a.abs() > b.abs() {
                    Ok(a)
                } else {
                    Ok(b)
                }
            }
        }
    }

    /// Process a liquidation event (call this from websocket handler)
    pub fn record_liquidation(&self, size_usd: f64, side: &str) {
        if let Ok(mut window) = self.liquidation_window.lock() {
            window.add(size_usd, side.to_string());
        }
    }

    /// Fetch recent liquidations via REST (fallback when no websocket)
    pub async fn fetch_recent_liquidations(&self, symbol: &str) -> Result<()> {
        let url = format!(
            "https://fapi.binance.com/fapi/v1/allForceOrders?symbol={}&limit=100",
            symbol
        );

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(());
        }

        #[derive(Deserialize)]
        struct ForceOrder {
            symbol: String,
            side: String,
            #[serde(rename = "origQty")]
            qty: String,
            price: String,
        }

        let orders: Vec<ForceOrder> = resp.json().await.unwrap_or_default();

        for order in orders {
            let qty: f64 = order.qty.parse().unwrap_or(0.0);
            let price: f64 = order.price.parse().unwrap_or(0.0);
            let size_usd = qty * price;
            self.record_liquidation(size_usd, &order.side);
        }

        Ok(())
    }
}

impl Default for AuxDataFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidation_window() {
        let mut window = LiquidationWindow::new(60);
        window.add(100_000.0, "SELL".to_string());
        window.add(50_000.0, "BUY".to_string());

        let score = window.score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_aux_fetcher_creation() {
        let fetcher = AuxDataFetcher::new();
        fetcher.record_liquidation(100_000.0, "SELL");

        let score = fetcher.liquidation_window.lock().unwrap().score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_cached_aux_freshness() {
        let data = MarketAux::default();
        let cached = CachedAux::new(data);

        // Immediately fresh
        assert!(cached.is_fresh(60));

        // Would be stale after TTL (can't easily test without sleep)
    }

    #[test]
    fn test_cached_aux_backoff() {
        let data = MarketAux::default();
        let mut cached = CachedAux::new(data);

        // Initially can retry
        assert!(cached.can_retry());

        // After failure, backoff kicks in
        cached.record_failure();
        assert_eq!(cached.backoff_secs(), 2); // 2^1

        cached.record_failure();
        assert_eq!(cached.backoff_secs(), 4); // 2^2

        cached.record_failure();
        assert_eq!(cached.backoff_secs(), 8); // 2^3

        // Caps at 300s
        for _ in 0..10 {
            cached.record_failure();
        }
        assert_eq!(cached.backoff_secs(), 256); // 2^8 = 256
    }

    #[test]
    fn test_cached_aux_success_resets() {
        let data = MarketAux::default();
        let mut cached = CachedAux::new(data);

        // Simulate failures
        cached.record_failure();
        cached.record_failure();
        assert_eq!(cached.fetch_failures, 2);

        // Success resets
        cached.record_success(data);
        assert_eq!(cached.fetch_failures, 0);
        assert!(cached.last_failure.is_none());
    }

    #[test]
    fn test_fetcher_with_custom_ttl() {
        let fetcher = AuxDataFetcher::with_ttl(120);
        assert_eq!(fetcher.cache_ttl_secs, 120);
    }
}
