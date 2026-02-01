use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::exchange::{Candle, Exchange};
use crate::exchange::signing::sign_binance;
use crate::state::{Config, Fill, now_ts};
use crate::strategy::{Action, MarketAux};

pub struct Binance {
    client: Client,
    base: String,
    api_key: Option<String>,
    api_secret: Option<String>,
}

impl Binance {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base: cfg.binance_base,
            api_key: cfg.api_key,
            api_secret: cfg.api_secret,
        })
    }

    fn as_kline_interval(granularity: u64) -> &'static str {
        match granularity {
            60 => "1m",
            300 => "5m",
            900 => "15m",
            3600 => "1h",
            _ => "1m",
        }
    }

    fn timestamp_ms() -> u64 {
        chrono::Utc::now().timestamp_millis() as u64
    }
}

#[derive(Deserialize, Debug)]
struct BinanceFundingRate {
    symbol: String,
    #[serde(rename = "fundingRate")]
    funding_rate: String,
    #[serde(rename = "fundingTime")]
    funding_time: u64,
}

#[derive(Deserialize, Debug)]
struct BinanceMarkPrice {
    symbol: String,
    #[serde(rename = "markPrice")]
    mark_price: String,
    #[serde(rename = "indexPrice")]
    index_price: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceOrderResponse {
    symbol: String,
    order_id: u64,
    client_order_id: String,
    transact_time: u64,
    price: String,
    orig_qty: String,
    executed_qty: String,
    status: String,
    #[serde(default)]
    fills: Vec<BinanceFill>,
}

#[derive(Deserialize, Debug)]
struct BinanceFill {
    price: String,
    qty: String,
    commission: String,
    #[serde(rename = "commissionAsset")]
    commission_asset: String,
}

#[derive(Deserialize, Debug)]
struct BinanceError {
    code: i64,
    msg: String,
}

#[async_trait::async_trait]
impl Exchange for Binance {
    async fn fetch_latest_candle(&self, symbol: &str, granularity: u64) -> Result<Candle> {
        let interval = Self::as_kline_interval(granularity);
        let url = format!("{}/api/v3/klines?symbol={}&interval={}&limit=1", self.base, symbol, interval);
        let resp = self.client.get(&url).send().await?;
        let data: Vec<Vec<serde_json::Value>> = resp.json().await?;
        let row = data.get(0).ok_or_else(|| anyhow!("empty kline"))?;
        Ok(Candle {
            ts: row[0].as_u64().unwrap_or(0) / 1000,
            o: row[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            h: row[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            l: row[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            c: row[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            v: row[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        })
    }

    async fn fetch_aux(&self, symbol: &str) -> Result<MarketAux> {
        // Fetch funding rate from Binance Futures API
        let funding_url = format!(
            "https://fapi.binance.com/fapi/v1/fundingRate?symbol={}&limit=1",
            symbol
        );
        let funding_rate = match self.client.get(&funding_url).send().await {
            Ok(resp) => {
                let data: Vec<BinanceFundingRate> = resp.json().await.unwrap_or_default();
                data.first()
                    .and_then(|f| f.funding_rate.parse::<f64>().ok())
                    .unwrap_or(0.0)
            }
            Err(_) => 0.0,
        };

        // Fetch mark price and index price for depeg calculation
        let mark_url = format!(
            "https://fapi.binance.com/fapi/v1/premiumIndex?symbol={}",
            symbol
        );
        let (mark_price, index_price) = match self.client.get(&mark_url).send().await {
            Ok(resp) => {
                let data: BinanceMarkPrice = resp.json().await.unwrap_or(BinanceMarkPrice {
                    symbol: symbol.to_string(),
                    mark_price: "0".to_string(),
                    index_price: "0".to_string(),
                });
                (
                    data.mark_price.parse::<f64>().unwrap_or(0.0),
                    data.index_price.parse::<f64>().unwrap_or(0.0),
                )
            }
            Err(_) => (0.0, 0.0),
        };

        // Calculate depeg as deviation from index price
        let stable_depeg = if index_price > 0.0 {
            (mark_price - index_price) / index_price
        } else {
            0.0
        };

        // Borrow rate is not directly available; use a proxy or set to 0
        let borrow_rate = 0.0;

        // Liquidation score: would need to aggregate from liquidation feed
        // For now, derive from funding rate magnitude as a proxy
        let liquidation_score = funding_rate.abs() * 1000.0;

        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(MarketAux {
            funding_rate,
            borrow_rate,
            liquidation_score,
            stable_depeg,
            fetch_ts: now_ts,
            has_funding: true,
            has_borrow: false,  // Not fetched from Binance
            has_liquidations: false,  // Using proxy
            has_depeg: stable_depeg != 0.0,
        })
    }

    async fn execute(&self, symbol: &str, action: Action, state: &crate::strategy::StrategyState) -> Result<Fill> {
        let (side, qty) = match action {
            Action::Buy { qty } => ("BUY", qty),
            Action::Sell { qty } => ("SELL", qty),
            Action::Close => {
                if state.portfolio.position > 0.0 {
                    ("SELL", state.portfolio.position.abs())
                } else if state.portfolio.position < 0.0 {
                    ("BUY", state.portfolio.position.abs())
                } else {
                    return Ok(Fill { price: 0.0, qty: 0.0, fee: 0.0, ts: now_ts() });
                }
            }
            Action::Hold => return Ok(Fill { price: 0.0, qty: 0.0, fee: 0.0, ts: now_ts() }),
        };

        let api_key = self.api_key.as_ref().ok_or_else(|| anyhow!("missing API_KEY"))?;
        let api_secret = self.api_secret.as_ref().ok_or_else(|| anyhow!("missing API_SECRET"))?;

        let timestamp = Self::timestamp_ms();
        let recv_window = 5000u64;

        // Build query string for signature
        let query = format!(
            "symbol={}&side={}&type=MARKET&quantity={:.8}&timestamp={}&recvWindow={}",
            symbol, side, qty, timestamp, recv_window
        );

        let signature = sign_binance(&query, api_secret);
        let signed_query = format!("{}&signature={}", query, signature);

        let url = format!("{}/api/v3/order?{}", self.base, signed_query);

        let resp = self.client
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            let err: BinanceError = serde_json::from_str(&body)
                .unwrap_or(BinanceError { code: -1, msg: body.clone() });
            return Err(anyhow!("Binance order failed: {} - {}", err.code, err.msg));
        }

        let order: BinanceOrderResponse = serde_json::from_str(&body)?;

        // Calculate fill from response
        let (total_qty, total_cost, total_fee) = order.fills.iter().fold(
            (0.0f64, 0.0f64, 0.0f64),
            |(q, c, f), fill| {
                let fq: f64 = fill.qty.parse().unwrap_or(0.0);
                let fp: f64 = fill.price.parse().unwrap_or(0.0);
                let fc: f64 = fill.commission.parse().unwrap_or(0.0);
                (q + fq, c + fq * fp, f + fc)
            },
        );

        let avg_price = if total_qty > 0.0 { total_cost / total_qty } else { 0.0 };
        let signed_qty = if side == "BUY" { total_qty } else { -total_qty };

        Ok(Fill {
            price: avg_price,
            qty: signed_qty,
            fee: total_fee,
            ts: order.transact_time / 1000,
        })
    }
}
