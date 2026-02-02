use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct Wal {
    file: File,
    path: String,
}

/// WAL entry types for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum WalEntry {
    #[serde(rename = "place_order")]
    PlaceOrder {
        ts: u64,
        intent_id: String,
        #[serde(default)]
        strategy_id: Option<String>,
        #[serde(default)]
        client_order_id: Option<String>,
        params_hash: String,
        symbol: String,
        side: String,
        qty: f64,
        #[serde(default)]
        fsync: bool,
    },
    #[serde(rename = "fill")]
    Fill {
        ts: u64,
        intent_id: String,
        params_hash: String,
        price: f64,
        qty: f64,
        fee: f64,
        #[serde(default)]
        fsync: bool,
    },
    #[serde(rename = "cancel")]
    Cancel {
        ts: u64,
        intent_id: String,
        params_hash: String,
        #[serde(default)]
        fsync: bool,
    },
    #[serde(rename = "snapshot")]
    Snapshot {
        ts: u64,
        strategy_id: String,
        cash: f64,
        position: f64,
        entry_price: f64,
        equity: f64,
        pnl: f64,
    },
}

/// Recovery state from WAL replay
#[derive(Debug, Clone, Default)]
pub struct RecoveryState {
    pub pending_orders: Vec<PendingOrder>,
    /// FIXED: Snapshots are now per-strategy to avoid cross-contamination
    pub snapshots_by_strategy: std::collections::HashMap<String, SnapshotData>,
    /// Deprecated: use snapshots_by_strategy instead
    pub last_snapshot: Option<SnapshotData>,
    /// Fills since the oldest snapshot (per-strategy filtering needed in caller)
    pub fills_since_snapshot: Vec<FillData>,
}

#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub intent_id: String,
    pub strategy_id: Option<String>,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub qty: f64,
    pub ts: u64,
}

#[derive(Debug, Clone)]
pub struct SnapshotData {
    pub ts: u64,
    pub strategy_id: String,
    pub cash: f64,
    pub position: f64,
    pub entry_price: f64,
    pub equity: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone)]
pub struct FillData {
    pub ts: u64,
    pub intent_id: String,
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
}

impl Wal {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file, path: path.to_string() })
    }

    pub fn append(&mut self, line: &str) -> std::io::Result<()> {
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()
    }

    pub fn append_json(&mut self, value: &Value) -> std::io::Result<()> {
        let line = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        self.append(&line)
    }

    pub fn append_entry(&mut self, entry: &WalEntry) -> std::io::Result<()> {
        let line = serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string());
        self.append(&line)
    }

    /// Read all lines from WAL file
    pub fn replay(path: &str) -> std::io::Result<Vec<String>> {
        if !Path::new(path).exists() {
            return Ok(vec![]);
        }
        let file = OpenOptions::new().read(true).open(path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().flatten().collect())
    }

    /// Parse WAL entries and build recovery state
    pub fn recover(path: &str) -> std::io::Result<RecoveryState> {
        let lines = Self::replay(path)?;
        let mut state = RecoveryState::default();
        let mut completed_intents: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in lines {
            if let Ok(entry) = serde_json::from_str::<WalEntry>(&line) {
                match entry {
                    WalEntry::PlaceOrder { ts, intent_id, strategy_id, client_order_id, symbol, side, qty, .. } => {
                        state.pending_orders.push(PendingOrder {
                            intent_id,
                            strategy_id,
                            client_order_id,
                            symbol,
                            side,
                            qty,
                            ts,
                        });
                    }
                    WalEntry::Fill { ts, intent_id, price, qty, fee, .. } => {
                        completed_intents.insert(intent_id.clone());
                        state.fills_since_snapshot.push(FillData {
                            ts,
                            intent_id,
                            price,
                            qty,
                            fee,
                        });
                    }
                    WalEntry::Cancel { intent_id, .. } => {
                        completed_intents.insert(intent_id);
                    }
                    WalEntry::Snapshot { ts, strategy_id, cash, position, entry_price, equity, pnl } => {
                        let snap = SnapshotData {
                            ts,
                            strategy_id: strategy_id.clone(),
                            cash,
                            position,
                            entry_price,
                            equity,
                            pnl,
                        };
                        state.snapshots_by_strategy.insert(strategy_id, snap.clone());
                        state.last_snapshot = Some(snap);
                        state.fills_since_snapshot.clear();
                    }
                }
                continue;
            }

            let parsed: Result<Value, _> = serde_json::from_str(&line);
            if let Ok(json) = parsed {
                let operation = json.get("operation").and_then(|v| v.as_str());

                match operation {
                    Some("place_order") => {
                        if let (Some(intent_id), Some(symbol), Some(side), Some(qty), Some(ts)) = (
                            json.get("intent_id").and_then(|v| v.as_str()),
                            json.get("symbol").and_then(|v| v.as_str()),
                            json.get("side").and_then(|v| v.as_str()),
                            json.get("qty").and_then(|v| v.as_f64()),
                            json.get("ts").and_then(|v| v.as_u64()),
                        ) {
                            let strategy_id = json
                                .get("strategy_id")
                                .and_then(|v| v.as_str())
                                .map(|v| v.to_string());
                            let client_order_id = json
                                .get("client_order_id")
                                .and_then(|v| v.as_str())
                                .map(|v| v.to_string());
                            state.pending_orders.push(PendingOrder {
                                intent_id: intent_id.to_string(),
                                strategy_id,
                                client_order_id,
                                symbol: symbol.to_string(),
                                side: side.to_string(),
                                qty,
                                ts,
                            });
                        }
                    }
                    Some("fill") => {
                        if let (Some(intent_id), Some(price), Some(qty), Some(fee), Some(ts)) = (
                            json.get("intent_id").and_then(|v| v.as_str()),
                            json.get("price").and_then(|v| v.as_f64()),
                            json.get("qty").and_then(|v| v.as_f64()),
                            json.get("fee").and_then(|v| v.as_f64()),
                            json.get("ts").and_then(|v| v.as_u64()),
                        ) {
                            completed_intents.insert(intent_id.to_string());
                            state.fills_since_snapshot.push(FillData {
                                ts,
                                intent_id: intent_id.to_string(),
                                price,
                                qty,
                                fee,
                            });
                        }
                    }
                    Some("cancel") => {
                        if let Some(intent_id) = json.get("intent_id").and_then(|v| v.as_str()) {
                            completed_intents.insert(intent_id.to_string());
                        }
                    }
                    Some("snapshot") => {
                        if let (
                            Some(ts),
                            Some(strategy_id),
                            Some(cash),
                            Some(position),
                            Some(entry_price),
                            Some(equity),
                            Some(pnl),
                        ) = (
                            json.get("ts").and_then(|v| v.as_u64()),
                            json.get("strategy_id").and_then(|v| v.as_str()),
                            json.get("cash").and_then(|v| v.as_f64()),
                            json.get("position").and_then(|v| v.as_f64()),
                            json.get("entry_price").and_then(|v| v.as_f64()),
                            json.get("equity").and_then(|v| v.as_f64()),
                            json.get("pnl").and_then(|v| v.as_f64()),
                        ) {
                            let snap = SnapshotData {
                                ts,
                                strategy_id: strategy_id.to_string(),
                                cash,
                                position,
                                entry_price,
                                equity,
                                pnl,
                            };
                            state.snapshots_by_strategy.insert(strategy_id.to_string(), snap.clone());
                            state.last_snapshot = Some(snap);
                            state.fills_since_snapshot.clear();
                        }
                    }
                    _ => {}
                }
            }
        }

        // Remove completed orders from pending
        state.pending_orders.retain(|o| !completed_intents.contains(&o.intent_id));

        Ok(state)
    }

    /// Write a snapshot entry for state persistence
    pub fn write_snapshot(&mut self, strategy_id: &str, portfolio: &crate::strategy::PortfolioState, pnl: f64) -> std::io::Result<()> {
        let entry = WalEntry::Snapshot {
            ts: crate::state::now_ts(),
            strategy_id: strategy_id.to_string(),
            cash: portfolio.cash,
            position: portfolio.position,
            entry_price: portfolio.entry_price,
            equity: portfolio.equity,
            pnl,
        };
        self.append_entry(&entry)
    }

    /// Truncate WAL after successful checkpoint
    pub fn truncate(&self) -> std::io::Result<()> {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    fn recovery_hash(state: &RecoveryState) -> u64 {
        let mut h = DefaultHasher::new();
        let mut pending = state.pending_orders.clone();
        pending.sort_by(|a, b| a.intent_id.cmp(&b.intent_id));
        for p in pending {
            p.intent_id.hash(&mut h);
            p.strategy_id.hash(&mut h);
            p.client_order_id.hash(&mut h);
            p.symbol.hash(&mut h);
            p.side.hash(&mut h);
            ((p.qty * 1e8) as i64).hash(&mut h);
            p.ts.hash(&mut h);
        }
        let mut snaps: Vec<_> = state.snapshots_by_strategy.values().cloned().collect();
        snaps.sort_by(|a, b| a.strategy_id.cmp(&b.strategy_id));
        for s in snaps {
            s.strategy_id.hash(&mut h);
            s.ts.hash(&mut h);
            ((s.cash * 1e8) as i64).hash(&mut h);
            ((s.position * 1e8) as i64).hash(&mut h);
            ((s.entry_price * 1e8) as i64).hash(&mut h);
            ((s.equity * 1e8) as i64).hash(&mut h);
            ((s.pnl * 1e8) as i64).hash(&mut h);
        }
        let mut fills = state.fills_since_snapshot.clone();
        fills.sort_by(|a, b| a.ts.cmp(&b.ts));
        for f in fills {
            f.intent_id.hash(&mut h);
            f.ts.hash(&mut h);
            ((f.price * 1e8) as i64).hash(&mut h);
            ((f.qty * 1e8) as i64).hash(&mut h);
            ((f.fee * 1e8) as i64).hash(&mut h);
        }
        h.finish()
    }

    #[test]
    fn test_wal_roundtrip() {
        let path = "/tmp/test_wal.log";
        let _ = fs::remove_file(path);

        {
            let mut wal = Wal::open(path).unwrap();
            wal.append_entry(&WalEntry::PlaceOrder {
                ts: 1234567890,
                intent_id: "I-1".to_string(),
                strategy_id: None,
                client_order_id: None,
                params_hash: "abc123".to_string(),
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                qty: 0.001,
                fsync: true,
            }).unwrap();

            wal.append_entry(&WalEntry::Fill {
                ts: 1234567891,
                intent_id: "I-1".to_string(),
                params_hash: "abc123".to_string(),
                price: 50000.0,
                qty: 0.001,
                fee: 0.05,
                fsync: true,
            }).unwrap();
        }

        let state = Wal::recover(path).unwrap();
        assert!(state.pending_orders.is_empty()); // fill completed the order
        assert_eq!(state.fills_since_snapshot.len(), 1);
        assert_eq!(state.fills_since_snapshot[0].price, 50000.0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_per_strategy_snapshot_recovery() {
        // CRITICAL: Tests that multiple strategies don't overwrite each other
        let path = "/tmp/test_wal_multi_strategy.log";
        let _ = fs::remove_file(path);

        {
            let mut wal = Wal::open(path).unwrap();

            // Strategy A snapshot
            wal.append_entry(&WalEntry::Snapshot {
                ts: 1000,
                strategy_id: "strategy_a".to_string(),
                cash: 10000.0,
                position: 0.5,
                entry_price: 50000.0,
                equity: 10500.0,
                pnl: 500.0,
            }).unwrap();

            // Strategy B snapshot (different values)
            wal.append_entry(&WalEntry::Snapshot {
                ts: 1000,
                strategy_id: "strategy_b".to_string(),
                cash: 8000.0,
                position: -0.3,
                entry_price: 51000.0,
                equity: 7800.0,
                pnl: -200.0,
            }).unwrap();
        }

        let state = Wal::recover(path).unwrap();

        // Both strategies should have their own snapshot
        assert_eq!(state.snapshots_by_strategy.len(), 2);

        let snap_a = state.snapshots_by_strategy.get("strategy_a").unwrap();
        assert_eq!(snap_a.position, 0.5);
        assert_eq!(snap_a.pnl, 500.0);

        let snap_b = state.snapshots_by_strategy.get("strategy_b").unwrap();
        assert_eq!(snap_b.position, -0.3);
        assert_eq!(snap_b.pnl, -200.0);

        // last_snapshot (deprecated) should be the last one written
        assert_eq!(state.last_snapshot.as_ref().unwrap().strategy_id, "strategy_b");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_fills_after_snapshot_preserved() {
        let path = "/tmp/test_wal_fills_after_snap.log";
        let _ = fs::remove_file(path);

        {
            let mut wal = Wal::open(path).unwrap();

            // Fill before snapshot
            wal.append_entry(&WalEntry::Fill {
                ts: 900,
                intent_id: "I-old".to_string(),
                params_hash: "old".to_string(),
                price: 49000.0,
                qty: 0.1,
                fee: 0.01,
                fsync: true,
            }).unwrap();

            // Snapshot
            wal.append_entry(&WalEntry::Snapshot {
                ts: 1000,
                strategy_id: "strat".to_string(),
                cash: 10000.0,
                position: 0.0,
                entry_price: 0.0,
                equity: 10000.0,
                pnl: 0.0,
            }).unwrap();

            // Fills after snapshot (should be preserved for replay)
            wal.append_entry(&WalEntry::Fill {
                ts: 1100,
                intent_id: "I-strat-1100".to_string(),
                params_hash: "new1".to_string(),
                price: 50000.0,
                qty: 0.05,
                fee: 0.005,
                fsync: true,
            }).unwrap();

            wal.append_entry(&WalEntry::Fill {
                ts: 1200,
                intent_id: "I-strat-1200".to_string(),
                params_hash: "new2".to_string(),
                price: 50500.0,
                qty: 0.03,
                fee: 0.003,
                fsync: true,
            }).unwrap();
        }

        let state = Wal::recover(path).unwrap();

        // Only fills AFTER snapshot should be preserved
        assert_eq!(state.fills_since_snapshot.len(), 2);
        assert_eq!(state.fills_since_snapshot[0].price, 50000.0);
        assert_eq!(state.fills_since_snapshot[1].price, 50500.0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_pending_orders_tracked() {
        let path = "/tmp/test_wal_pending.log";
        let _ = fs::remove_file(path);

        {
            let mut wal = Wal::open(path).unwrap();

            // Order placed but not filled
            wal.append_entry(&WalEntry::PlaceOrder {
                ts: 1000,
                intent_id: "I-pending".to_string(),
                strategy_id: None,
                client_order_id: None,
                params_hash: "pending".to_string(),
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                qty: 0.1,
                fsync: true,
            }).unwrap();

            // Order placed and filled
            wal.append_entry(&WalEntry::PlaceOrder {
                ts: 1001,
                intent_id: "I-filled".to_string(),
                strategy_id: None,
                client_order_id: None,
                params_hash: "filled".to_string(),
                symbol: "BTCUSDT".to_string(),
                side: "SELL".to_string(),
                qty: 0.05,
                fsync: true,
            }).unwrap();

            wal.append_entry(&WalEntry::Fill {
                ts: 1002,
                intent_id: "I-filled".to_string(),
                params_hash: "filled".to_string(),
                price: 50000.0,
                qty: 0.05,
                fee: 0.005,
                fsync: true,
            }).unwrap();
        }

        let state = Wal::recover(path).unwrap();

        // Only unfilled order should be pending
        assert_eq!(state.pending_orders.len(), 1);
        assert_eq!(state.pending_orders[0].intent_id, "I-pending");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_recovery_hash_determinism() {
        let path = "/tmp/test_wal_hash.log";
        let _ = fs::remove_file(path);

        {
            let mut wal = Wal::open(path).unwrap();
            wal.append_entry(&WalEntry::PlaceOrder {
                ts: 1000,
                intent_id: "I-1".to_string(),
                strategy_id: Some("s-1".to_string()),
                client_order_id: Some("CID-1".to_string()),
                params_hash: "h1".to_string(),
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                qty: 0.1,
                fsync: true,
            }).unwrap();
            wal.append_entry(&WalEntry::Snapshot {
                ts: 1000,
                strategy_id: "s-1".to_string(),
                cash: 1000.0,
                position: 0.1,
                entry_price: 50000.0,
                equity: 1005.0,
                pnl: 5.0,
            }).unwrap();
            wal.append_entry(&WalEntry::Fill {
                ts: 1001,
                intent_id: "I-1".to_string(),
                params_hash: "h1".to_string(),
                price: 50000.0,
                qty: 0.1,
                fee: 0.01,
                fsync: true,
            }).unwrap();
        }

        let state1 = Wal::recover(path).unwrap();
        let state2 = Wal::recover(path).unwrap();
        assert_eq!(recovery_hash(&state1), recovery_hash(&state2));

        let _ = fs::remove_file(path);
    }
}
