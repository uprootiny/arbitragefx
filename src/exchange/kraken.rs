use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

use crate::exchange::{Candle, Exchange};
use crate::exchange::signing::sign_kraken;
use crate::state::{Config, Fill, now_ts};
use crate::strategy::{Action, MarketAux};

pub struct Kraken {
    client: Client,
    base: String,
    api_key: Option<String>,
    api_secret: Option<String>,
}

impl Kraken {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base: cfg.kraken_base,
            api_key: cfg.api_key,
            api_secret: cfg.api_secret,
        })
    }

    fn as_kraken_interval(granularity: u64) -> u64 {
        match granularity {
            60 => 1,
            300 => 5,
            900 => 15,
            3600 => 60,
            _ => 1,
        }
    }

    fn nonce() -> u64 {
        chrono::Utc::now().timestamp_millis() as u64
    }

    fn to_kraken_pair(symbol: &str) -> String {
        if symbol.starts_with("BTC") {
            symbol.replacen("BTC", "XBT", 1)
        } else {
            symbol.to_string()
        }
    }

    /// Query order status to get real fill information
    async fn query_order(&self, txid: &str) -> Result<KrakenOrderInfo> {
        let api_key = self.api_key.as_ref().ok_or_else(|| anyhow!("missing API_KEY"))?;
        let api_secret = self.api_secret.as_ref().ok_or_else(|| anyhow!("missing API_SECRET"))?;

        let nonce = Self::nonce();
        let post_data = format!("nonce={}&txid={}", nonce, txid);
        let uri_path = "/0/private/QueryOrders";

        let signature = sign_kraken(uri_path, nonce, &post_data, api_secret)
            .map_err(|e| anyhow!("signing error: {}", e))?;

        let url = format!("{}{}", self.base, uri_path);

        let resp = self.client
            .post(&url)
            .header("API-Key", api_key)
            .header("API-Sign", &signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data)
            .send()
            .await?;

        let body = resp.text().await?;
        let result: KrakenResp<HashMap<String, KrakenOrderInfo>> = serde_json::from_str(&body)?;

        if !result.error.is_empty() {
            return Err(anyhow!("Kraken query error: {:?}", result.error));
        }

        result.result
            .and_then(|r| r.into_values().next())
            .ok_or_else(|| anyhow!("no order info"))
    }

    /// Poll order until filled or timeout
    async fn wait_for_fill(&self, txid: &str, max_wait_secs: u64) -> Result<KrakenOrderInfo> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(max_wait_secs);

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow!("fill timeout"));
            }

            match self.query_order(txid).await {
                Ok(info) => {
                    if info.status == "closed" || info.vol_exec.parse::<f64>().unwrap_or(0.0) > 0.0 {
                        return Ok(info);
                    }
                }
                Err(_) => {}
            }

            sleep(Duration::from_millis(500)).await;
        }
    }
}

#[derive(Deserialize, Debug)]
struct KrakenResp<T> {
    error: Vec<String>,
    result: Option<T>,
}

#[derive(Deserialize, Debug)]
struct KrakenOrderResult {
    txid: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct KrakenOrderInfo {
    status: String,
    #[serde(default)]
    vol: String,
    #[serde(default)]
    vol_exec: String,
    #[serde(default)]
    cost: String,
    #[serde(default)]
    fee: String,
    #[serde(default)]
    price: String,
    #[serde(default)]
    descr: KrakenOrderDescr,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct KrakenOrderDescr {
    #[serde(default)]
    pair: String,
    #[serde(default)]
    #[serde(rename = "type")]
    side: String,
    #[serde(default)]
    ordertype: String,
    #[serde(default)]
    price: String,
}

#[derive(Deserialize, Debug, Clone)]
struct KrakenTickerInfo {
    a: Vec<String>,
    b: Vec<String>,
    c: Vec<String>,
}

#[async_trait::async_trait]
impl Exchange for Kraken {
    async fn fetch_latest_candle(&self, symbol: &str, granularity: u64) -> Result<Candle> {
        let pair = Self::to_kraken_pair(symbol);
        let interval = Self::as_kraken_interval(granularity);
        let url = format!("{}/0/public/OHLC?pair={}&interval={}", self.base, pair, interval);
        let resp = self.client.get(&url).send().await?;
        let data: KrakenResp<serde_json::Value> = resp.json().await?;

        if !data.error.is_empty() {
            return Err(anyhow!("Kraken error: {:?}", data.error));
        }

        let result = data.result.ok_or_else(|| anyhow!("missing result"))?;
        let arr = result.as_object().ok_or_else(|| anyhow!("invalid kraken response"))?;
        let (_, series) = arr.iter().find(|(k, _)| *k != "last").ok_or_else(|| anyhow!("missing series"))?;
        let candles = series.as_array().ok_or_else(|| anyhow!("bad series"))?;
        let row = candles.last().ok_or_else(|| anyhow!("empty"))?;
        let row = row.as_array().ok_or_else(|| anyhow!("bad row"))?;

        Ok(Candle {
            ts: row[0].as_u64().unwrap_or(0),
            o: row[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            h: row[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            l: row[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            c: row[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            v: row[6].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        })
    }

    async fn fetch_aux(&self, symbol: &str) -> Result<MarketAux> {
        let pair = Self::to_kraken_pair(symbol);

        // Fetch ticker for spread calculation
        let ticker_url = format!("{}/0/public/Ticker?pair={}", self.base, pair);
        let (bid, ask) = match self.client.get(&ticker_url).send().await {
            Ok(resp) => {
                let data: KrakenResp<HashMap<String, KrakenTickerInfo>> = resp.json().await.unwrap_or(KrakenResp {
                    error: vec![],
                    result: None,
                });
                if let Some(result) = data.result {
                    if let Some(info) = result.values().next() {
                        let bid: f64 = info.b.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let ask: f64 = info.a.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        (bid, ask)
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0)
                }
            }
            Err(_) => (0.0, 0.0),
        };

        // Kraken spot doesn't have perpetual funding
        // Use spread as a proxy for market stress
        let spread = if bid > 0.0 { (ask - bid) / bid } else { 0.0 };

        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(MarketAux {
            funding_rate: 0.0,
            borrow_rate: spread * 10.0, // scale spread to meaningful range
            liquidation_score: 0.0,
            stable_depeg: 0.0,
            fetch_ts: now_ts,
            has_funding: false,  // Kraken spot doesn't have perpetual funding
            has_borrow: spread > 0.0,  // Using spread as proxy
            has_liquidations: false,
            has_depeg: false,
        })
    }

    async fn execute(&self, symbol: &str, action: Action, state: &crate::strategy::StrategyState) -> Result<Fill> {
        let (side, qty) = match action {
            Action::Buy { qty } => ("buy", qty),
            Action::Sell { qty } => ("sell", qty),
            Action::Close => {
                if state.portfolio.position > 0.0 {
                    ("sell", state.portfolio.position.abs())
                } else if state.portfolio.position < 0.0 {
                    ("buy", state.portfolio.position.abs())
                } else {
                    return Ok(Fill { price: 0.0, qty: 0.0, fee: 0.0, ts: now_ts() });
                }
            }
            Action::Hold => return Ok(Fill { price: 0.0, qty: 0.0, fee: 0.0, ts: now_ts() }),
        };

        let api_key = self.api_key.as_ref().ok_or_else(|| anyhow!("missing API_KEY"))?;
        let api_secret = self.api_secret.as_ref().ok_or_else(|| anyhow!("missing API_SECRET"))?;

        let pair = Self::to_kraken_pair(symbol);
        let nonce = Self::nonce();

        let post_data = format!(
            "nonce={}&ordertype=market&type={}&volume={:.8}&pair={}",
            nonce, side, qty, pair
        );

        let uri_path = "/0/private/AddOrder";
        let signature = sign_kraken(uri_path, nonce, &post_data, api_secret)
            .map_err(|e| anyhow!("signing error: {}", e))?;

        let url = format!("{}{}", self.base, uri_path);

        let resp = self.client
            .post(&url)
            .header("API-Key", api_key)
            .header("API-Sign", &signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Kraken HTTP error: {} - {}", status, body));
        }

        let result: KrakenResp<KrakenOrderResult> = serde_json::from_str(&body)?;

        if !result.error.is_empty() {
            return Err(anyhow!("Kraken order failed: {:?}", result.error));
        }

        let order = result.result.ok_or_else(|| anyhow!("missing order result"))?;
        let txid = order.txid.first().ok_or_else(|| anyhow!("no txid"))?;

        // Poll for actual fill data
        let fill_info = self.wait_for_fill(txid, 30).await?;

        let vol_exec: f64 = fill_info.vol_exec.parse().unwrap_or(0.0);
        let cost: f64 = fill_info.cost.parse().unwrap_or(0.0);
        let fee: f64 = fill_info.fee.parse().unwrap_or(0.0);

        let avg_price = if vol_exec > 0.0 { cost / vol_exec } else { 0.0 };
        let signed_qty = if side == "buy" { vol_exec } else { -vol_exec };

        Ok(Fill {
            price: avg_price,
            qty: signed_qty,
            fee,
            ts: now_ts(),
        })
    }
}
