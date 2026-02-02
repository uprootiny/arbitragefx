use reqwest::Client;
use serde::Deserialize;

use crate::exchange::signing::sign_binance;
use super::types::{OrderRequest, OrderResponse, Side, OrderType};
use super::unified::UnifiedAdapter;

pub struct BinanceAdapter {
    client: Client,
    base: String,
    api_key: String,
    api_secret: String,
    runtime: tokio::runtime::Handle,
}

impl BinanceAdapter {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            client: Client::new(),
            base: std::env::var("BINANCE_BASE")
                .unwrap_or_else(|_| "https://api.binance.com".to_string()),
            api_key,
            api_secret,
            runtime: tokio::runtime::Handle::current(),
        }
    }

    fn timestamp_ms() -> u64 {
        chrono::Utc::now().timestamp_millis() as u64
    }

    async fn place_order_async(&self, req: OrderRequest) -> Result<OrderResponse, String> {
        let timestamp = Self::timestamp_ms();
        let recv_window = 5000u64;

        let side = match req.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        let order_type = match req.order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
        };

        let mut query = format!(
            "symbol={}&side={}&type={}&quantity={:.8}&newClientOrderId={}&timestamp={}&recvWindow={}",
            req.symbol, side, order_type, req.qty, req.client_id, timestamp, recv_window
        );

        if let (OrderType::Limit, Some(price)) = (req.order_type, req.price) {
            query.push_str(&format!("&price={:.8}&timeInForce=GTC", price));
        }

        let signature = sign_binance(&query, &self.api_secret)
            .map_err(|e| format!("signing failed: {}", e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/api/v3/order?{}", self.base, signed_query);

        let resp = self.client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read body failed: {}", e))?;

        if !status.is_success() {
            #[derive(Deserialize)]
            struct BinanceError { code: i64, msg: String }
            let err: BinanceError = serde_json::from_str(&body)
                .unwrap_or(BinanceError { code: -1, msg: body.clone() });
            return Err(format!("Binance error {}: {}", err.code, err.msg));
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BinanceNewOrder {
            order_id: u64,
            status: String,
        }

        let order: BinanceNewOrder = serde_json::from_str(&body)
            .map_err(|e| format!("parse error: {}", e))?;

        Ok(OrderResponse {
            order_id: order.order_id.to_string(),
            status: order.status,
        })
    }

    async fn cancel_order_async(&self, order_id: &str) -> Result<(), String> {
        let timestamp = Self::timestamp_ms();
        let symbol = std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());

        let query = format!(
            "symbol={}&orderId={}&timestamp={}&recvWindow=5000",
            symbol, order_id, timestamp
        );

        let signature = sign_binance(&query, &self.api_secret)
            .map_err(|e| format!("signing failed: {}", e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/api/v3/order?{}", self.base, signed_query);

        let resp = self.client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("cancel failed: {}", body));
        }

        Ok(())
    }

    async fn cancel_all_async(&self) -> Result<(), String> {
        let timestamp = Self::timestamp_ms();
        let symbol = std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());

        let query = format!(
            "symbol={}&timestamp={}&recvWindow=5000",
            symbol, timestamp
        );

        let signature = sign_binance(&query, &self.api_secret)
            .map_err(|e| format!("signing failed: {}", e))?;
        let signed_query = format!("{}&signature={}", query, signature);
        let url = format!("{}/api/v3/openOrders?{}", self.base, signed_query);

        let resp = self.client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("cancel all failed: {}", body));
        }

        Ok(())
    }
}

impl UnifiedAdapter for BinanceAdapter {
    fn place_order(&mut self, req: OrderRequest) -> Result<OrderResponse, String> {
        self.runtime.block_on(self.place_order_async(req))
    }

    fn cancel_order(&mut self, order_id: &str) -> Result<(), String> {
        self.runtime.block_on(self.cancel_order_async(order_id))
    }

    fn cancel_all(&mut self) -> Result<(), String> {
        self.runtime.block_on(self.cancel_all_async())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = BinanceAdapter::timestamp_ms();
        assert!(ts > 1700000000000); // sanity check
    }
}
