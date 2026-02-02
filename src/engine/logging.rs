//! Ledger + Journal logging architecture.
//!
//! Two distinct streams:
//! - **Ledger (WAL)**: Deterministic audit trail for replay + validation
//! - **Journal**: Narratable, summarizable event stream for humans + AI agents
//!
//! Design principles:
//! - Ledger is canonical truth; journal references ledger by seq/ids
//! - Every causal chain is linkable: intent_id → decision_id → client_order_id → exchange_order_id → fill_id
//! - Reason codes bridge truth and narratability

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

// =============================================================================
// Sequence Counter (monotonic, process-local)
// =============================================================================

static LEDGER_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn next_seq() -> u64 {
    LEDGER_SEQ.fetch_add(1, Ordering::SeqCst)
}

pub fn set_seq(val: u64) {
    LEDGER_SEQ.store(val, Ordering::SeqCst);
}

pub fn current_seq() -> u64 {
    LEDGER_SEQ.load(Ordering::SeqCst)
}

// =============================================================================
// Timestamps
// =============================================================================

pub fn ts_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

// =============================================================================
// ID Types (newtypes for clarity)
// =============================================================================

/// Intent ID: I-<strategy>-<ts>-<seq>
pub fn intent_id(strategy: &str, ts: u64, seq: u64) -> String {
    format!("I-{}-{}-{}", strategy, ts, seq)
}

/// Decision ID: D-<strategy>-<ts>-<seq>
pub fn decision_id(strategy: &str, ts: u64, seq: u64) -> String {
    format!("D-{}-{}-{}", strategy, ts, seq)
}

/// Client Order ID: CID-<strategy>-<ts>-<seq>
pub fn client_order_id(strategy: &str, ts: u64, seq: u64) -> String {
    format!("CID-{}-{}-{}", strategy, ts, seq)
}

/// Fill ID (if exchange doesn't provide one)
pub fn fill_id(exchange_oid: &str, ts: u64, n: u64) -> String {
    format!("F-{}-{}-{}", exchange_oid, ts, n)
}

// =============================================================================
// Reason Codes (the bridge between truth and narratability)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    // Data integrity
    FeedStaleMarket,
    FeedStaleAux,
    FeedMissingAux,
    FeedErrorAux,

    // Uncertainty / mindfulness
    RegimeUnknown,
    RegimeReflexive,
    DriftModerate,
    DriftSevere,

    // Risk limits
    LossCapHit,
    PositionCapHit,
    NotionalCapHit,
    OrderRateLimit,
    CooldownActive,

    // Execution safety
    SlippageTooHigh,
    SpreadTooWide,
    PendingTooLong,
    ReconcileFailed,
    UnmatchedFill,

    // System
    ManualHalt,
    CircuitBreaker,
}

impl ReasonCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FeedStaleMarket => "feed_stale_market",
            Self::FeedStaleAux => "feed_stale_aux",
            Self::FeedMissingAux => "feed_missing_aux",
            Self::FeedErrorAux => "feed_error_aux",
            Self::RegimeUnknown => "regime_unknown",
            Self::RegimeReflexive => "regime_reflexive",
            Self::DriftModerate => "drift_moderate",
            Self::DriftSevere => "drift_severe",
            Self::LossCapHit => "loss_cap_hit",
            Self::PositionCapHit => "position_cap_hit",
            Self::NotionalCapHit => "notional_cap_hit",
            Self::OrderRateLimit => "order_rate_limit",
            Self::CooldownActive => "cooldown_active",
            Self::SlippageTooHigh => "slippage_too_high",
            Self::SpreadTooWide => "spread_too_wide",
            Self::PendingTooLong => "pending_too_long",
            Self::ReconcileFailed => "reconcile_failed",
            Self::UnmatchedFill => "unmatched_fill",
            Self::ManualHalt => "manual_halt",
            Self::CircuitBreaker => "circuit_breaker",
        }
    }
}

// =============================================================================
// Risk Verdict
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "v", content = "d")]
pub enum RiskVerdict {
    Allow,
    Deny,
    Throttle { size_mult: f32 },
}

// =============================================================================
// Intent (minimal action space)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "k", content = "v")]
pub enum Intent {
    Hold,
    Flat,
    TargetPosition { symbol: String, target_qty: f64 },
    DeltaPosition { symbol: String, delta_qty: f64 },
}

// =============================================================================
// Aux Data Names
// =============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuxName {
    Funding,
    PremiumIndex,
    Borrow,
    Depeg,
    Liquidations,
}

// =============================================================================
// Order Types
// =============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelScope {
    One,
    AllSymbol,
    AllVenue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Liquidity {
    Maker,
    Taker,
}

// =============================================================================
// Ledger Record (canonical, replayable)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerRecord {
    pub v: u16,                      // schema version
    pub seq: u64,                    // monotonically increasing
    pub ts_ms: u64,                  // epoch millis
    #[serde(flatten)]
    pub kind: LedgerKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_hash: Option<String>,  // optional tamper-evidence
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LedgerKind {
    // Lifecycle
    Boot {
        run_id: String,
        strategy_id: String,
        git_sha: Option<String>,
    },
    Shutdown {
        reason: String,
    },

    // Market truth
    MarketCandle {
        venue: String,
        symbol: String,
        tf: String,
        close_ts_ms: u64,
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        v: f64,
    },

    // Aux data samples
    AuxSample {
        venue: String,
        symbol: String,
        name: AuxName,
        as_of_ms: u64,
        value: Option<f64>,
        err: Option<String>,
    },

    // Agent/strategy intent (proposed action)
    IntentProposed {
        intent_id: String,
        strategy_id: String,
        as_of_ms: u64,
        intent: Intent,
        confidence: Option<f32>,
        state_hash: Option<String>,
    },

    // Risk gate (authoritative)
    RiskDecision {
        decision_id: String,
        intent_id: String,
        verdict: RiskVerdict,
        reason_codes: Vec<ReasonCode>,
        details: serde_json::Value,
    },

    // Order commands
    OrderSubmit {
        decision_id: String,
        client_order_id: String,
        venue: String,
        symbol: String,
        side: Side,
        qty: f64,
        limit_price: Option<f64>,
        tif: Option<String>,
    },

    OrderCancel {
        decision_id: Option<String>,
        client_order_id: Option<String>,
        exchange_order_id: Option<String>,
        venue: String,
        symbol: String,
        scope: CancelScope,
    },

    // Exchange truth
    ExecAck {
        client_order_id: String,
        exchange_order_id: String,
        venue: String,
        symbol: String,
    },

    ExecReject {
        client_order_id: String,
        venue: String,
        symbol: String,
        code: Option<String>,
        msg: String,
    },

    ExecFill {
        exchange_order_id: String,
        venue: String,
        symbol: String,
        fill_id: String,
        qty: f64,
        price: f64,
        fee: f64,
        fee_ccy: Option<String>,
        liquidity: Option<Liquidity>,
    },

    ExecCancelAck {
        exchange_order_id: String,
        venue: String,
        symbol: String,
    },

    // Periodic verification
    Checkpoint {
        strategy_id: String,
        cash: f64,
        position: f64,
        entry_price: f64,
        equity: f64,
        pnl: f64,
        state_hash: String,
    },
}

// =============================================================================
// Journal Record (summarizable, agent-friendly)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    System,
    Market,
    Aux,
    Strategy,
    Risk,
    Exec,
    Recon,
    Drift,
    Backtest,
    Trial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalRecord {
    pub v: u16,
    pub ts_ms: u64,
    pub level: Level,
    pub domain: Domain,
    pub msg: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<Ref>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum Ref {
    Seq(u64),
    Intent(String),
    Decision(String),
    ClientOrder(String),
    ExchangeOrder(String),
    Fill(String),
}

// =============================================================================
// Ledger Writer
// =============================================================================

pub struct LedgerWriter {
    writer: Mutex<BufWriter<File>>,
    run_id: String,
}

impl LedgerWriter {
    pub fn new(path: &Path, run_id: String) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            run_id,
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn write(&self, kind: LedgerKind, fsync: bool) -> std::io::Result<u64> {
        let seq = next_seq();
        let record = LedgerRecord {
            v: 1,
            seq,
            ts_ms: ts_ms(),
            kind,
            chain_hash: None, // TODO: implement chain hashing
        };
        let line = serde_json::to_string(&record)?;
        let mut w = self.writer.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "ledger writer lock poisoned")
        })?;
        writeln!(w, "{}", line)?;
        if fsync {
            w.flush()?;
            w.get_ref().sync_all()?;
        }
        Ok(seq)
    }
}

// =============================================================================
// Journal Writer
// =============================================================================

pub struct JournalWriter {
    writer: Mutex<BufWriter<File>>,
    min_level: Level,
}

impl JournalWriter {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let min_level = match std::env::var("LOG_LEVEL").as_deref() {
            Ok("trace") => Level::Trace,
            Ok("debug") => Level::Debug,
            Ok("info") => Level::Info,
            Ok("warn") => Level::Warn,
            Ok("error") => Level::Error,
            _ => Level::Info,
        };
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            min_level,
        })
    }

    pub fn write(&self, level: Level, domain: Domain, msg: &str, refs: Vec<Ref>, fields: serde_json::Value) -> std::io::Result<()> {
        if level < self.min_level {
            return Ok(());
        }
        let record = JournalRecord {
            v: 1,
            ts_ms: ts_ms(),
            level,
            domain,
            msg: msg.to_string(),
            refs,
            fields,
        };
        let line = serde_json::to_string(&record)?;
        let mut w = self.writer.lock().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "journal writer lock poisoned")
        })?;
        writeln!(w, "{}", line)?;
        // Journal doesn't need fsync
        Ok(())
    }

    pub fn info(&self, domain: Domain, msg: &str, fields: serde_json::Value) -> std::io::Result<()> {
        self.write(Level::Info, domain, msg, vec![], fields)
    }

    pub fn warn(&self, domain: Domain, msg: &str, reason_codes: &[ReasonCode], fields: serde_json::Value) -> std::io::Result<()> {
        let mut f = fields;
        if let serde_json::Value::Object(ref mut map) = f {
            map.insert("reason_codes".to_string(), serde_json::json!(reason_codes));
        }
        self.write(Level::Warn, domain, msg, vec![], f)
    }

    pub fn with_refs(&self, level: Level, domain: Domain, msg: &str, refs: Vec<Ref>, fields: serde_json::Value) -> std::io::Result<()> {
        self.write(level, domain, msg, refs, fields)
    }
}

// =============================================================================
// Tick Summary (periodic journal checkpoint)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSummary {
    pub symbol: String,
    pub regime: String,
    pub drift: DriftSummary,
    pub health: HealthSummary,
    pub exposure: ExposureSummary,
    pub actions: ActionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftSummary {
    pub severity: String,
    pub scores: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub market_age_ms: u64,
    pub aux_age_ms: u64,
    pub errors_5m: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureSummary {
    pub position: f64,
    pub notional: f64,
    pub leverage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSummary {
    pub intents: u64,
    pub orders: u64,
    pub fills: u64,
    pub halts: u64,
}

impl JournalWriter {
    pub fn tick_summary(&self, summary: &TickSummary) -> std::io::Result<()> {
        self.info(Domain::System, "tick_summary", serde_json::to_value(summary).unwrap_or_default())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seq_increments() {
        let s1 = next_seq();
        let s2 = next_seq();
        assert!(s2 > s1);
    }

    #[test]
    fn test_intent_id_format() {
        let id = intent_id("churn-0", 1706745600, 42);
        assert_eq!(id, "I-churn-0-1706745600-42");
    }

    #[test]
    fn test_decision_id_format() {
        let id = decision_id("churn-0", 1706745600, 42);
        assert_eq!(id, "D-churn-0-1706745600-42");
    }

    #[test]
    fn test_reason_code_serde() {
        let code = ReasonCode::DriftModerate;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"drift_moderate\"");
    }

    #[test]
    fn test_risk_verdict_serde() {
        let v1 = RiskVerdict::Allow;
        let v2 = RiskVerdict::Throttle { size_mult: 0.5 };

        let j1 = serde_json::to_string(&v1).unwrap();
        let j2 = serde_json::to_string(&v2).unwrap();

        assert!(j1.contains("Allow"));
        assert!(j2.contains("0.5"));
    }

    #[test]
    fn test_ledger_record_serde() {
        let record = LedgerRecord {
            v: 1,
            seq: 42,
            ts_ms: 1706745600000,
            kind: LedgerKind::Checkpoint {
                strategy_id: "churn-0".to_string(),
                cash: 1000.0,
                position: 0.1,
                entry_price: 50000.0,
                equity: 1005.0,
                pnl: 5.0,
                state_hash: "abc123".to_string(),
            },
            chain_hash: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"seq\":42"));
        assert!(json.contains("\"type\":\"Checkpoint\""));
    }

    #[test]
    fn test_journal_record_serde() {
        let record = JournalRecord {
            v: 1,
            ts_ms: 1706745600000,
            level: Level::Info,
            domain: Domain::Risk,
            msg: "Position denied".to_string(),
            refs: vec![Ref::Intent("I-churn-0-123-1".to_string())],
            fields: serde_json::json!({"reason": "loss cap hit"}),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"domain\":\"risk\""));
    }

    #[test]
    fn test_level_ordering() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
    }
}
