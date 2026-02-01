use anyhow::Result;
use rusqlite::{params, Connection};

use crate::state::StrategyInstance;

pub struct StateStore {
    conn: Connection,
}

impl StateStore {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self { conn: Connection::open(path)? })
    }

    pub fn init(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS metrics (
                ts INTEGER NOT NULL,
                strategy_id TEXT NOT NULL,
                equity REAL NOT NULL,
                pnl REAL NOT NULL,
                wins INTEGER NOT NULL,
                losses INTEGER NOT NULL,
                max_drawdown REAL NOT NULL
            );
            COMMIT;",
        )?;
        Ok(())
    }

    pub fn persist_snapshot(&mut self, ts: u64, strategies: &[StrategyInstance]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for inst in strategies {
            let s = &inst.state;
            tx.execute(
                "INSERT INTO metrics (ts, strategy_id, equity, pnl, wins, losses, max_drawdown)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    ts as i64,
                    inst.id,
                    s.portfolio.equity,
                    s.metrics.pnl,
                    s.metrics.wins as i64,
                    s.metrics.losses as i64,
                    s.metrics.max_drawdown
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
