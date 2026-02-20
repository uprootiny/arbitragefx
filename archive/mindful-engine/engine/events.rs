//! Canonical typed events for deterministic replay.

use serde::{Deserialize, Serialize};

/// Unique event identifier for idempotency
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId {
    pub source: String,
    pub seq: u64,
}

/// Timestamp in milliseconds
pub type Timestamp = u64;

/// All events that can affect system state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Market(MarketEvent),
    Exec(ExecEvent),
    Sys(SysEvent),
}

impl Event {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Event::Market(e) => e.timestamp(),
            Event::Exec(e) => e.timestamp(),
            Event::Sys(e) => e.timestamp(),
        }
    }

    pub fn symbol(&self) -> Option<&str> {
        match self {
            Event::Market(e) => Some(e.symbol()),
            Event::Exec(e) => Some(e.symbol()),
            Event::Sys(_) => None,
        }
    }
}

/// Market data events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    Candle {
        ts: Timestamp,
        symbol: String,
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        v: f64,
    },
    Trade {
        ts: Timestamp,
        symbol: String,
        price: f64,
        qty: f64,
        side: TradeSide,
    },
    Funding {
        ts: Timestamp,
        symbol: String,
        rate: f64,
        next_ts: Timestamp,
    },
    Liquidation {
        ts: Timestamp,
        symbol: String,
        side: TradeSide,
        qty: f64,
        price: f64,
    },
    BookUpdate {
        ts: Timestamp,
        symbol: String,
        bid: f64,
        ask: f64,
        bid_qty: f64,
        ask_qty: f64,
    },
}

impl MarketEvent {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketEvent::Candle { ts, .. } => *ts,
            MarketEvent::Trade { ts, .. } => *ts,
            MarketEvent::Funding { ts, .. } => *ts,
            MarketEvent::Liquidation { ts, .. } => *ts,
            MarketEvent::BookUpdate { ts, .. } => *ts,
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            MarketEvent::Candle { symbol, .. } => symbol,
            MarketEvent::Trade { symbol, .. } => symbol,
            MarketEvent::Funding { symbol, .. } => symbol,
            MarketEvent::Liquidation { symbol, .. } => symbol,
            MarketEvent::BookUpdate { symbol, .. } => symbol,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Execution events from exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecEvent {
    OrderAck {
        ts: Timestamp,
        symbol: String,
        client_id: String,
        order_id: String,
    },
    Fill {
        ts: Timestamp,
        symbol: String,
        client_id: String,
        order_id: String,
        fill_id: String,
        price: f64,
        qty: f64,
        fee: f64,
        side: TradeSide,
    },
    PartialFill {
        ts: Timestamp,
        symbol: String,
        client_id: String,
        order_id: String,
        fill_id: String,
        price: f64,
        qty: f64,
        remaining: f64,
        fee: f64,
        side: TradeSide,
    },
    CancelAck {
        ts: Timestamp,
        symbol: String,
        client_id: String,
        order_id: String,
    },
    Reject {
        ts: Timestamp,
        symbol: String,
        client_id: String,
        reason: String,
    },
}

impl ExecEvent {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            ExecEvent::OrderAck { ts, .. } => *ts,
            ExecEvent::Fill { ts, .. } => *ts,
            ExecEvent::PartialFill { ts, .. } => *ts,
            ExecEvent::CancelAck { ts, .. } => *ts,
            ExecEvent::Reject { ts, .. } => *ts,
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            ExecEvent::OrderAck { symbol, .. } => symbol,
            ExecEvent::Fill { symbol, .. } => symbol,
            ExecEvent::PartialFill { symbol, .. } => symbol,
            ExecEvent::CancelAck { symbol, .. } => symbol,
            ExecEvent::Reject { symbol, .. } => symbol,
        }
    }

    pub fn client_id(&self) -> &str {
        match self {
            ExecEvent::OrderAck { client_id, .. } => client_id,
            ExecEvent::Fill { client_id, .. } => client_id,
            ExecEvent::PartialFill { client_id, .. } => client_id,
            ExecEvent::CancelAck { client_id, .. } => client_id,
            ExecEvent::Reject { client_id, .. } => client_id,
        }
    }
}

/// System / infrastructure events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SysEvent {
    Timer {
        ts: Timestamp,
        name: String,
    },
    Reconnect {
        ts: Timestamp,
        source: String,
    },
    DataStale {
        ts: Timestamp,
        symbol: String,
        last_seen: Timestamp,
    },
    Health {
        ts: Timestamp,
        status: HealthStatus,
    },
    Halt {
        ts: Timestamp,
        reason: HaltReason,
    },
}

impl SysEvent {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            SysEvent::Timer { ts, .. } => *ts,
            SysEvent::Reconnect { ts, .. } => *ts,
            SysEvent::DataStale { ts, .. } => *ts,
            SysEvent::Health { ts, .. } => *ts,
            SysEvent::Halt { ts, .. } => *ts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Ok,
    Degraded,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HaltReason {
    KillSwitch,
    MaxErrors { count: u32 },
    MaxDrawdown { pct: f64 },
    DataStale { symbol: String, secs: u64 },
    SpreadTooWide { symbol: String, spread_pct: f64 },
    PriceJump { symbol: String, pct: f64 },
    Manual { reason: String },
}

/// Commands emitted by the reducer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    PlaceOrder {
        symbol: String,
        client_id: String,
        side: TradeSide,
        qty: f64,
        price: Option<f64>,
    },
    CancelOrder {
        symbol: String,
        client_id: String,
    },
    CancelAll {
        symbol: Option<String>,
    },
    Halt {
        reason: HaltReason,
    },
    Log {
        level: LogLevel,
        msg: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
