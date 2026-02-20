use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use crate::exchange::signing::sign_binance;

#[derive(Debug, Clone)]
pub struct FillEvent {
    pub client_id: String,
    pub order_id: String,
    pub fill_id: String,
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
    pub ts: u64,
    pub side: String,
}

#[derive(Debug, Deserialize)]
struct ListenKeyResp {
    listen_key: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WsExecReport {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "x")]
    exec_type: String,
    #[serde(rename = "X")]
    order_status: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "i")]
    order_id: u64,
    #[serde(rename = "c")]
    client_id: String,
    #[serde(rename = "t")]
    trade_id: u64,
    #[serde(rename = "l")]
    last_qty: String,
    #[serde(rename = "L")]
    last_price: String,
    #[serde(rename = "n")]
    commission: String,
    #[serde(rename = "T")]
    trade_time: u64,
}

#[derive(Debug, Deserialize)]
struct WsMessage {
    #[serde(rename = "e")]
    event_type: Option<String>,
    #[serde(flatten)]
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Trade {
    id: u64,
    #[serde(rename = "orderId")]
    order_id: u64,
    #[serde(rename = "clientOrderId")]
    client_id: String,
    price: String,
    qty: String,
    commission: String,
    #[serde(rename = "time")]
    time_ms: u64,
    #[serde(rename = "isBuyer")]
    is_buyer: bool,
}

pub async fn start_ws_listener(
    api_key: String,
    base: String,
    sender: mpsc::Sender<FillEvent>,
) -> Result<()> {
    let client = Client::new();
    let listen_key = get_listen_key(&client, &api_key, &base).await?;

    let ws_url = format!("wss://stream.binance.com:9443/ws/{}", listen_key);
    let (ws, _) = tokio_tungstenite::connect_async(ws_url).await?;
    let (mut _write, mut read) = ws.split();

    // Keepalive task
    let keep_client = client.clone();
    let keep_api_key = api_key.clone();
    let keep_base = base.clone();
    tokio::spawn(async move {
        loop {
            let _ = keepalive_listen_key(&keep_client, &keep_api_key, &keep_base).await;
            sleep(Duration::from_secs(30 * 60)).await;
        }
    });

    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if let Ok(text) = msg.into_text() {
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    if ws_msg.event_type.as_deref() == Some("executionReport") {
                        if let Ok(report) = serde_json::from_value::<WsExecReport>(ws_msg.data) {
                            if report.exec_type == "TRADE" {
                                let qty: f64 = report.last_qty.parse().unwrap_or(0.0);
                                let price: f64 = report.last_price.parse().unwrap_or(0.0);
                                let fee: f64 = report.commission.parse().unwrap_or(0.0);
                                if qty > 0.0 && price > 0.0 {
                                    let _ = sender
                                        .send(FillEvent {
                                            client_id: report.client_id.clone(),
                                            order_id: report.order_id.to_string(),
                                            fill_id: format!("trade-{}", report.trade_id),
                                            price,
                                            qty,
                                            fee,
                                            ts: report.trade_time / 1000,
                                            side: report.side.clone(),
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn start_poll_fallback(
    api_key: String,
    api_secret: String,
    base: String,
    symbol: String,
    sender: mpsc::Sender<FillEvent>,
    poll_secs: u64,
) -> Result<()> {
    let client = Client::new();
    let mut last_ts: u64 = 0;

    loop {
        let trades = fetch_my_trades(&client, &api_key, &api_secret, &base, &symbol, last_ts).await;
        if let Ok(trades) = trades {
            for t in trades {
                last_ts = last_ts.max(t.time_ms + 1);
                let qty: f64 = t.qty.parse().unwrap_or(0.0);
                let price: f64 = t.price.parse().unwrap_or(0.0);
                let fee: f64 = t.commission.parse().unwrap_or(0.0);
                if qty > 0.0 && price > 0.0 {
                    let _ = sender
                        .send(FillEvent {
                            client_id: t.client_id.clone(),
                            order_id: t.order_id.to_string(),
                            fill_id: format!("trade-{}", t.id),
                            price,
                            qty,
                            fee,
                            ts: t.time_ms / 1000,
                            side: if t.is_buyer {
                                "BUY".to_string()
                            } else {
                                "SELL".to_string()
                            },
                        })
                        .await;
                }
            }
        }
        sleep(Duration::from_secs(poll_secs)).await;
    }
}

async fn get_listen_key(client: &Client, api_key: &str, base: &str) -> Result<String> {
    let url = format!("{}/api/v3/userDataStream", base);
    let resp = client
        .post(url)
        .header("X-MBX-APIKEY", api_key)
        .send()
        .await?;
    let body = resp.text().await?;
    let parsed: ListenKeyResp = serde_json::from_str(&body)?;
    Ok(parsed.listen_key)
}

async fn keepalive_listen_key(client: &Client, api_key: &str, base: &str) -> Result<()> {
    let url = format!("{}/api/v3/userDataStream", base);
    let _ = client
        .put(url)
        .header("X-MBX-APIKEY", api_key)
        .send()
        .await?;
    Ok(())
}

async fn fetch_my_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    base: &str,
    symbol: &str,
    start_time_ms: u64,
) -> Result<Vec<Trade>> {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let mut query = format!("symbol={}&timestamp={}&recvWindow=5000", symbol, timestamp);
    if start_time_ms > 0 {
        query.push_str(&format!("&startTime={}", start_time_ms));
    }
    let signature = sign_binance(&query, api_secret).map_err(|e| anyhow::anyhow!(e))?;
    let signed = format!("{}&signature={}", query, signature);
    let url = format!("{}/api/v3/myTrades?{}", base, signed);

    let resp = client
        .get(url)
        .header("X-MBX-APIKEY", api_key)
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("myTrades error: {}", body));
    }

    let data: Vec<Trade> = resp.json().await?;
    Ok(data)
}
