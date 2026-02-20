use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::exchange::signing::sign_binance;

#[derive(Debug, Clone)]
pub struct SpotBalance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
}

#[derive(Debug, Clone)]
pub struct SpotOrder {
    pub client_order_id: String,
    pub order_id: String,
    pub orig_qty: f64,
    pub executed_qty: f64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct FuturesPosition {
    pub symbol: String,
    pub position_amt: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_profit: f64,
}

#[derive(Clone)]
pub struct BinanceReconcileClient {
    client: Client,
    spot_base: String,
    fapi_base: String,
    api_key: String,
    api_secret: String,
}

impl BinanceReconcileClient {
    pub fn new(spot_base: String, fapi_base: String, api_key: String, api_secret: String) -> Self {
        Self {
            client: Client::new(),
            spot_base,
            fapi_base,
            api_key,
            api_secret,
        }
    }

    fn timestamp_ms() -> u64 {
        chrono::Utc::now().timestamp_millis() as u64
    }

    pub async fn fetch_open_orders(&self, symbol: &str) -> Result<Vec<SpotOrder>> {
        let timestamp = Self::timestamp_ms();
        let query = format!("symbol={}&timestamp={}&recvWindow=5000", symbol, timestamp);
        let signature = sign_binance(&query, &self.api_secret).map_err(|e| anyhow!(e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/api/v3/openOrders?{}", self.spot_base, signed_query);

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("openOrders failed: {}", body));
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OpenOrder {
            order_id: u64,
            client_order_id: String,
            orig_qty: String,
            executed_qty: String,
            status: String,
        }

        let orders: Vec<OpenOrder> = resp.json().await?;
        Ok(orders
            .into_iter()
            .map(|o| SpotOrder {
                client_order_id: o.client_order_id,
                order_id: o.order_id.to_string(),
                orig_qty: o.orig_qty.parse().unwrap_or(0.0),
                executed_qty: o.executed_qty.parse().unwrap_or(0.0),
                status: o.status,
            })
            .collect())
    }

    pub async fn fetch_spot_balances(&self) -> Result<Vec<SpotBalance>> {
        let timestamp = Self::timestamp_ms();
        let query = format!("timestamp={}&recvWindow=5000", timestamp);
        let signature = sign_binance(&query, &self.api_secret).map_err(|e| anyhow!(e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/api/v3/account?{}", self.spot_base, signed_query);

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("account failed: {}", body));
        }

        #[derive(Deserialize)]
        struct AccountBalance {
            asset: String,
            free: String,
            locked: String,
        }

        #[derive(Deserialize)]
        struct AccountResponse {
            balances: Vec<AccountBalance>,
        }

        let account: AccountResponse = resp.json().await?;
        Ok(account
            .balances
            .into_iter()
            .map(|b| SpotBalance {
                asset: b.asset,
                free: b.free.parse().unwrap_or(0.0),
                locked: b.locked.parse().unwrap_or(0.0),
            })
            .collect())
    }

    pub async fn fetch_futures_positions(&self, symbol: &str) -> Result<Vec<FuturesPosition>> {
        let timestamp = Self::timestamp_ms();
        let query = format!("timestamp={}&recvWindow=5000", timestamp);
        let signature = sign_binance(&query, &self.api_secret).map_err(|e| anyhow!(e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/fapi/v2/positionRisk?{}", self.fapi_base, signed_query);

        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("positionRisk failed: {}", body));
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PositionRisk {
            symbol: String,
            position_amt: String,
            entry_price: String,
            mark_price: String,
            unrealized_profit: String,
        }

        let positions: Vec<PositionRisk> = resp.json().await?;
        Ok(positions
            .into_iter()
            .filter(|p| p.symbol == symbol)
            .map(|p| FuturesPosition {
                symbol: p.symbol,
                position_amt: p.position_amt.parse().unwrap_or(0.0),
                entry_price: p.entry_price.parse().unwrap_or(0.0),
                mark_price: p.mark_price.parse().unwrap_or(0.0),
                unrealized_profit: p.unrealized_profit.parse().unwrap_or(0.0),
            })
            .collect())
    }
}
