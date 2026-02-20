//! Structured logging for AI-guided trading systems.
//!
//! Design goals:
//! 1. Multi-level granularity (TRACE → FATAL)
//! 2. Domain-specific categories for filtering
//! 3. Summarization-friendly periodic checkpoints
//! 4. Replay/audit support via deterministic timestamps and state hashes
//! 5. AI agent guidance fields (intent, reason, confidence, alternatives)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// =============================================================================
// Log Levels
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl Level {
    pub fn from_env() -> Self {
        match std::env::var("LOG_LEVEL").as_deref() {
            Ok("trace") => Level::Trace,
            Ok("debug") => Level::Debug,
            Ok("info") => Level::Info,
            Ok("warn") => Level::Warn,
            Ok("error") => Level::Error,
            Ok("fatal") => Level::Fatal,
            _ => Level::Info,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
            Level::Fatal => "fatal",
        }
    }
}

// =============================================================================
// Log Domains (categories for filtering)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Market,   // Price data, candles, indicators
    Strategy, // Signal generation, decisions
    Risk,     // Guard checks, limits, ethics
    Exec,     // Order lifecycle, submissions
    Fill,     // Fill processing, reconciliation
    Drift,    // Distribution drift detection
    System,   // Startup, shutdown, recovery
    Profile,  // Performance profiling
    Audit,    // Replay/audit trail entries
    Agent,    // AI agent guidance logs
}

impl Domain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Domain::Market => "market",
            Domain::Strategy => "strategy",
            Domain::Risk => "risk",
            Domain::Exec => "exec",
            Domain::Fill => "fill",
            Domain::Drift => "drift",
            Domain::System => "system",
            Domain::Profile => "profile",
            Domain::Audit => "audit",
            Domain::Agent => "agent",
        }
    }

    pub fn is_enabled(&self) -> bool {
        // Check LOG_DOMAINS env var (comma-separated list or "all")
        match std::env::var("LOG_DOMAINS").as_deref() {
            Ok("all") | Err(_) => true,
            Ok(domains) => domains.split(',').any(|d| d.trim() == self.as_str()),
        }
    }
}

// =============================================================================
// Sequence counter for ordering
// =============================================================================

static LOG_SEQ: AtomicU64 = AtomicU64::new(0);
static PROFILE_SEQ: AtomicU64 = AtomicU64::new(0);
static RUN_CONTEXT: OnceLock<RunContext> = OnceLock::new();

fn next_seq() -> u64 {
    LOG_SEQ.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug)]
struct RunContext {
    run_id: String,
    events: Mutex<BufWriter<File>>,
    trace: Mutex<BufWriter<File>>,
    metrics: Mutex<BufWriter<File>>,
}

fn ensure_run_context() -> &'static RunContext {
    RUN_CONTEXT.get_or_init(|| {
        let run_id = std::env::var("RUN_ID")
            .unwrap_or_else(|_| format!("r-{}-{}", ts_epoch_ms(), process::id()));
        let base = std::env::var("LOG_DIR").unwrap_or_else(|_| "out/runs".to_string());
        let mut run_dir = PathBuf::from(base);
        run_dir.push(&run_id);
        if let Err(err) = create_dir_all(&run_dir) {
            eprintln!("[log] failed to create run dir: {}", err);
        }
        let events_path = run_dir.join("events.jsonl");
        let trace_path = run_dir.join("trace.jsonl");
        let metrics_path = run_dir.join("metrics.jsonl");
        let manifest_path = run_dir.join("manifest.json");

        let _ = std::fs::write(
            manifest_path,
            json!({
                "run_id": run_id,
                "ts": ts_now(),
                "pid": process::id(),
                "log_dir": run_dir.to_string_lossy(),
            })
            .to_string(),
        );

        let events = File::create(events_path).unwrap_or_else(|err| {
            eprintln!("[log] failed to create events log: {}", err);
            File::create("/tmp/arbitragefx-events.jsonl").expect("events fallback")
        });
        let trace = File::create(trace_path).unwrap_or_else(|err| {
            eprintln!("[log] failed to create trace log: {}", err);
            File::create("/tmp/arbitragefx-trace.jsonl").expect("trace fallback")
        });
        let metrics = File::create(metrics_path).unwrap_or_else(|err| {
            eprintln!("[log] failed to create metrics log: {}", err);
            File::create("/tmp/arbitragefx-metrics.jsonl").expect("metrics fallback")
        });

        RunContext {
            run_id,
            events: Mutex::new(BufWriter::new(events)),
            trace: Mutex::new(BufWriter::new(trace)),
            metrics: Mutex::new(BufWriter::new(metrics)),
        }
    })
}

fn sanitize_fields(mut fields: Map<String, Value>) -> Map<String, Value> {
    let redacted = Value::String("[REDACTED]".to_string());
    for key in [
        "authorization",
        "Authorization",
        "X-MBX-APIKEY",
        "api_key",
        "signature",
    ] {
        if fields.contains_key(key) {
            fields.insert(key.to_string(), redacted.clone());
        }
    }
    fields
}

fn split_fields(mut fields: Map<String, Value>) -> (Map<String, Value>, Map<String, Value>) {
    let mut top = Map::new();
    for key in ["intent_id", "corr_id", "strategy_id", "symbol", "msg"] {
        if let Some(value) = fields.remove(key) {
            top.insert(key.to_string(), value);
        }
    }
    (top, fields)
}

fn write_line(writer: &Mutex<BufWriter<File>>, line: &str) {
    if let Ok(mut w) = writer.lock() {
        let _ = writeln!(w, "{}", line);
    }
}

// =============================================================================
// Core logging functions
// =============================================================================

/// RFC3339 timestamp with milliseconds
pub fn ts_now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Epoch milliseconds (for replay correlation)
pub fn ts_epoch_ms() -> u64 {
    Utc::now().timestamp_millis() as u64
}

/// Emit a structured log entry
pub fn log(level: Level, domain: Domain, event: &str, fields: Map<String, Value>) {
    let min_level = Level::from_env();
    if level < min_level || !domain.is_enabled() {
        return;
    }

    emit_record(level, domain.as_str(), event, fields);
}

/// Legacy compatibility: json_log with module name
pub fn json_log(module: &str, mut fields: Map<String, Value>) {
    fields.insert("ts".to_string(), Value::String(ts_now()));
    fields.insert("module".to_string(), Value::String(module.to_string()));
    emit_record(Level::Info, module, module, fields);
}

fn emit_record(level: Level, component: &str, event: &str, fields: Map<String, Value>) {
    let ctx = ensure_run_context();
    let fields = sanitize_fields(fields);
    let (mut top, data) = split_fields(fields);

    let msg = top.remove("msg").unwrap_or(Value::String(String::new()));
    let mut entry = Map::new();
    entry.insert("ts".to_string(), json!(ts_now()));
    entry.insert("run_id".to_string(), json!(ctx.run_id.clone()));
    entry.insert("seq".to_string(), json!(next_seq()));
    entry.insert("lvl".to_string(), json!(level.as_str().to_uppercase()));
    entry.insert("component".to_string(), json!(component));
    entry.insert("event".to_string(), json!(event));
    entry.insert("msg".to_string(), msg);
    for (k, v) in top {
        entry.insert(k, v);
    }
    entry.insert("data".to_string(), Value::Object(data));

    let line = Value::Object(entry).to_string();
    if component == "metrics" || event.starts_with("metrics.") {
        write_line(&ctx.metrics, &line);
    }
    match level {
        Level::Trace | Level::Debug => write_line(&ctx.trace, &line),
        _ => write_line(&ctx.events, &line),
    }
    println!("{}", line);
}

// =============================================================================
// AI Agent Guidance Logs
// =============================================================================

/// Log a decision point for AI agent understanding
pub fn log_decision(
    strategy_id: &str,
    intent: &str,
    reason: &str,
    confidence: f64,
    alternatives: &[(&str, f64)], // (action, score) pairs
    state_hash: Option<&str>,
) {
    let alts: Vec<Value> = alternatives
        .iter()
        .map(|(action, score)| json!({"action": action, "score": score}))
        .collect();

    log(
        Level::Info,
        Domain::Agent,
        "decision",
        obj(&[
            ("strategy_id", v_str(strategy_id)),
            ("intent", v_str(intent)),
            ("reason", v_str(reason)),
            ("confidence", v_num(confidence)),
            ("alternatives", Value::Array(alts)),
            ("state_hash", state_hash.map(v_str).unwrap_or(Value::Null)),
        ]),
    );
}

/// Log context for AI agent to understand market state
pub fn log_market_context(
    symbol: &str,
    price: f64,
    z_momentum: f64,
    z_vol: f64,
    funding_rate: f64,
    regime: &str,
    drift_severity: &str,
) {
    log(
        Level::Debug,
        Domain::Agent,
        "market_context",
        obj(&[
            ("symbol", v_str(symbol)),
            ("price", v_num(price)),
            ("z_momentum", v_num(z_momentum)),
            ("z_vol", v_num(z_vol)),
            ("funding_rate", v_num(funding_rate)),
            ("regime", v_str(regime)),
            ("drift_severity", v_str(drift_severity)),
        ]),
    );
}

/// Log reasoning chain for AI transparency
pub fn log_reasoning(strategy_id: &str, steps: &[&str]) {
    log(
        Level::Debug,
        Domain::Agent,
        "reasoning",
        obj(&[
            ("strategy_id", v_str(strategy_id)),
            (
                "steps",
                Value::Array(steps.iter().map(|s| v_str(s)).collect()),
            ),
        ]),
    );
}

// =============================================================================
// Audit Trail Logs
// =============================================================================

/// Log an audit entry for replay verification
pub fn log_audit(event_type: &str, state_hash: &str, input_hash: &str, output_hash: &str) {
    log(
        Level::Info,
        Domain::Audit,
        event_type,
        obj(&[
            ("state_hash", v_str(state_hash)),
            ("input_hash", v_str(input_hash)),
            ("output_hash", v_str(output_hash)),
        ]),
    );
}

/// Log WAL checkpoint for recovery
pub fn log_checkpoint(
    strategy_id: &str,
    state_hash: &str,
    wal_offset: u64,
    pnl: f64,
    position: f64,
) {
    log(
        Level::Info,
        Domain::Audit,
        "checkpoint",
        obj(&[
            ("strategy_id", v_str(strategy_id)),
            ("state_hash", v_str(state_hash)),
            ("wal_offset", json!(wal_offset)),
            ("pnl", v_num(pnl)),
            ("position", v_num(position)),
        ]),
    );
}

// =============================================================================
// Summarization Logs
// =============================================================================

/// Periodic summary for aggregation
pub fn log_periodic_summary(
    period_secs: u64,
    strategies: &[StrategySummary],
    total_pnl: f64,
    total_trades: u64,
    drift_events: u64,
) {
    let strat_json: Vec<Value> = strategies
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "pnl": s.pnl,
                "position": s.position,
                "trades": s.trades,
                "win_rate": s.win_rate,
            })
        })
        .collect();

    log(
        Level::Info,
        Domain::System,
        "periodic_summary",
        obj(&[
            ("period_secs", json!(period_secs)),
            ("strategies", Value::Array(strat_json)),
            ("total_pnl", v_num(total_pnl)),
            ("total_trades", json!(total_trades)),
            ("drift_events", json!(drift_events)),
        ]),
    );
}

#[derive(Debug, Clone)]
pub struct StrategySummary {
    pub id: String,
    pub pnl: f64,
    pub position: f64,
    pub trades: u64,
    pub win_rate: f64,
}

/// Session summary on shutdown
pub fn log_session_summary(
    duration_secs: u64,
    total_pnl: f64,
    max_drawdown: f64,
    total_trades: u64,
    win_rate: f64,
    halts: u64,
    drift_triggers: u64,
) {
    log(
        Level::Info,
        Domain::System,
        "session_summary",
        obj(&[
            ("duration_secs", json!(duration_secs)),
            ("total_pnl", v_num(total_pnl)),
            ("max_drawdown", v_num(max_drawdown)),
            ("total_trades", json!(total_trades)),
            ("win_rate", v_num(win_rate)),
            ("halts", json!(halts)),
            ("drift_triggers", json!(drift_triggers)),
        ]),
    );
}

// =============================================================================
// Domain-Specific Logging Helpers
// =============================================================================

pub fn log_candle(symbol: &str, ts: u64, o: f64, h: f64, l: f64, c: f64, v: f64) {
    log(
        Level::Trace,
        Domain::Market,
        "candle",
        obj(&[
            ("symbol", v_str(symbol)),
            ("candle_ts", json!(ts)),
            ("o", v_num(o)),
            ("h", v_num(h)),
            ("l", v_num(l)),
            ("c", v_num(c)),
            ("v", v_num(v)),
        ]),
    );
}

pub fn log_signal(strategy_id: &str, action: &str, score: f64, reason: &str) {
    log(
        Level::Debug,
        Domain::Strategy,
        "signal",
        obj(&[
            ("strategy_id", v_str(strategy_id)),
            ("action", v_str(action)),
            ("score", v_num(score)),
            ("reason", v_str(reason)),
        ]),
    );
}

pub fn log_risk_check(check: &str, result: &str, value: f64, threshold: f64) {
    log(
        Level::Debug,
        Domain::Risk,
        "guard",
        obj(&[
            ("check", v_str(check)),
            ("result", v_str(result)),
            ("value", v_num(value)),
            ("threshold", v_num(threshold)),
        ]),
    );
}

pub fn log_order_submit(
    client_id: &str,
    strategy_id: &str,
    symbol: &str,
    side: &str,
    qty: f64,
    state_hash: &str,
) {
    log(
        Level::Info,
        Domain::Exec,
        "order_submit",
        obj(&[
            ("client_id", v_str(client_id)),
            ("strategy_id", v_str(strategy_id)),
            ("symbol", v_str(symbol)),
            ("side", v_str(side)),
            ("qty", v_num(qty)),
            ("state_hash", v_str(state_hash)),
        ]),
    );
}

pub fn log_fill(
    client_id: &str,
    strategy_id: &str,
    price: f64,
    qty: f64,
    fee: f64,
    realized_pnl: f64,
) {
    log(
        Level::Info,
        Domain::Fill,
        "fill",
        obj(&[
            ("client_id", v_str(client_id)),
            ("strategy_id", v_str(strategy_id)),
            ("price", v_num(price)),
            ("qty", v_num(qty)),
            ("fee", v_num(fee)),
            ("realized_pnl", v_num(realized_pnl)),
        ]),
    );
}

pub fn log_drift(severity: &str, multiplier: f64, metrics: &[(&str, f64)]) {
    let mets: Map<String, Value> = metrics
        .iter()
        .map(|(k, v)| (k.to_string(), v_num(*v)))
        .collect();

    log(
        Level::Warn,
        Domain::Drift,
        "drift_detected",
        obj(&[
            ("severity", v_str(severity)),
            ("multiplier", v_num(multiplier)),
            ("metrics", Value::Object(mets)),
        ]),
    );
}

// =============================================================================
// Utility Functions (legacy compatibility)
// =============================================================================

pub fn params_hash(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}

pub fn obj(pairs: &[(&str, Value)]) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), v.clone());
    }
    map
}

pub fn v_str(s: &str) -> Value {
    Value::String(s.to_string())
}

pub fn v_num(n: f64) -> Value {
    json!(n)
}

// =============================================================================
// Profiling Scope
// =============================================================================

/// Profiling scope that emits structured timing on drop.
pub struct ProfileScope {
    domain: Domain,
    label: &'static str,
    context: Option<Map<String, Value>>,
    started: Instant,
    enabled: bool,
}

impl ProfileScope {
    pub fn new(_module: &'static str, label: &'static str) -> Self {
        let enabled = Self::should_sample();
        Self {
            domain: Domain::Profile,
            label,
            context: None,
            started: Instant::now(),
            enabled,
        }
    }

    pub fn with_context(
        _module: &'static str,
        label: &'static str,
        fields: &[(&str, Value)],
    ) -> Self {
        let enabled = Self::should_sample();
        Self {
            domain: Domain::Profile,
            label,
            context: if enabled { Some(obj(fields)) } else { None },
            started: Instant::now(),
            enabled,
        }
    }

    fn should_sample() -> bool {
        std::env::var("PROFILE_SAMPLE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|p| {
                if p >= 1.0 {
                    true
                } else if p <= 0.0 {
                    false
                } else {
                    let seq = PROFILE_SEQ.fetch_add(1, Ordering::SeqCst);
                    let bucket = (seq % 10_000) as f64 / 10_000.0;
                    bucket < p
                }
            })
            .unwrap_or(true)
    }
}

impl Drop for ProfileScope {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let mut fields = self.context.take().unwrap_or_default();
        fields.insert("label".to_string(), v_str(self.label));
        fields.insert("elapsed_ms".to_string(), v_num(elapsed_ms));
        log(Level::Trace, self.domain, "profile", fields);
    }
}

// =============================================================================
// Log Aggregator for Periodic Summaries
// =============================================================================

static AGGREGATOR: OnceLock<Mutex<LogAggregator>> = OnceLock::new();

fn get_aggregator() -> &'static Mutex<LogAggregator> {
    AGGREGATOR.get_or_init(|| Mutex::new(LogAggregator::new()))
}

struct LogAggregator {
    trades: u64,
    fills: u64,
    drift_events: u64,
    risk_blocks: u64,
    last_flush: Instant,
    flush_interval_secs: u64,
}

impl LogAggregator {
    fn new() -> Self {
        Self {
            trades: 0,
            fills: 0,
            drift_events: 0,
            risk_blocks: 0,
            last_flush: Instant::now(),
            flush_interval_secs: std::env::var("LOG_FLUSH_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        }
    }

    fn increment(&mut self, event: &str) {
        match event {
            "trade" => self.trades += 1,
            "fill" => self.fills += 1,
            "drift" => self.drift_events += 1,
            "risk_block" => self.risk_blocks += 1,
            _ => {}
        }
    }

    fn maybe_flush(&mut self) -> Option<(u64, u64, u64, u64)> {
        if self.last_flush.elapsed().as_secs() >= self.flush_interval_secs {
            let result = (self.trades, self.fills, self.drift_events, self.risk_blocks);
            self.trades = 0;
            self.fills = 0;
            self.drift_events = 0;
            self.risk_blocks = 0;
            self.last_flush = Instant::now();
            Some(result)
        } else {
            None
        }
    }
}

/// Call periodically to emit aggregated stats
pub fn tick_aggregator() {
    if let Ok(mut agg) = get_aggregator().lock() {
        if let Some((trades, fills, drift, blocks)) = agg.maybe_flush() {
            log(
                Level::Info,
                Domain::System,
                "aggregated_stats",
                obj(&[
                    ("trades", json!(trades)),
                    ("fills", json!(fills)),
                    ("drift_events", json!(drift)),
                    ("risk_blocks", json!(blocks)),
                ]),
            );
        }
    }
}

/// Increment a counter in the aggregator
pub fn agg_increment(event: &str) {
    if let Ok(mut agg) = get_aggregator().lock() {
        agg.increment(event);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_ordering() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
        assert!(Level::Error < Level::Fatal);
    }

    #[test]
    fn test_params_hash_deterministic() {
        let h1 = params_hash("test-input");
        let h2 = params_hash("test-input");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_params_hash_different_inputs() {
        let h1 = params_hash("input-a");
        let h2 = params_hash("input-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_obj_helper() {
        let m = obj(&[("key", v_str("value")), ("num", v_num(42.0))]);
        assert_eq!(m.get("key").unwrap(), "value");
        assert_eq!(m.get("num").unwrap(), 42.0);
    }

    #[test]
    fn test_seq_increments() {
        let s1 = next_seq();
        let s2 = next_seq();
        assert!(s2 > s1);
    }
}
