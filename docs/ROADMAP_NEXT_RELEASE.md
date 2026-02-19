# Roadmap: v0.2.0 — Smoke Tests & Reality Grounding

**Goal:** Every claim the system makes is backed by a passing smoke test.
No new features until the existing core is ground-truthed.

---

## Phase 0: Smoke Test Harness (this commit)

- [x] `tests/smoke.rs` — end-to-end smoke tests that run in `cargo test`
- [x] `scripts/smoke.sh` — shell-level smoke tests for binaries and data
- [x] CI gate: nothing merges without `cargo test` + `scripts/smoke.sh` green

### Smoke Test Categories

| # | Test | What It Proves |
|---|------|---------------|
| S01 | `cargo build --release` succeeds | Code compiles |
| S02 | `cargo test` all pass | Unit/integration tests hold |
| S03 | Backtest on btc_real_1h.csv produces output | Core pipeline works |
| S04 | Backtest output has all 12 strategies | Strategy factory works |
| S05 | All equity values > 0 | Invariant I001 holds |
| S06 | All max_drawdown <= 5% | Invariant I002 holds |
| S07 | Deterministic: two runs produce identical output | Replay property |
| S08 | Each real CSV has valid schema (11 columns) | Data integrity |
| S09 | Each real CSV has SHA256 matching manifest | Data provenance |
| S10 | No strategy beats buy-hold in bull market | Sanity check (H007) |
| S11 | Friction > 0 for all strategies with trades > 0 | Fee accounting |
| S12 | Trade count correlates with friction | H008 regression |

---

## Phase 1: Kill the Lies (3-5 commits)

### 1a. Epistemic server serves real data
- `EpistemicState::from_system()` actually scans source files
- Smoke test: response contains real file paths and line counts
- Smoke test: stratum counts change when code changes

### 1b. Config snapshotting
- `run_backtest()` returns a `RunResult` struct including config hash
- Config serialized to JSON alongside output
- Smoke test: two runs with different ENV produce different config hashes

### 1c. Data manifest enforcement
- `data/manifest.json` with SHA256 for each CSV
- Backtest refuses to run on unmanifested data
- Smoke test: backtest on tampered CSV fails with clear error

---

## Phase 2: Strategy Ground Truth (3-5 commits)

### 2a. Strategy unit tests on known data
- Hand-crafted 10-candle sequences with known correct actions
- Test: SimpleMomentum buys on up-trend, sells on down-trend
- Test: SimpleMomentum holds during vol pause
- Test: CarryOpportunistic enters on funding imbalance

### 2b. Regression snapshots
- Golden output files for each regime backtest
- Smoke test: output matches golden file (or diff is explained)

### 2c. Composable strategies: use them or delete them
- Wire `strategies.rs` into a backtest binary
- Run on all 4 regimes
- Compare to SimpleMomentum baseline
- Decision: keep (if any beat baseline) or delete (if none do)

---

## Phase 3: Frequency Loosening (2-3 commits)

### 3a. Minimum hold period parameter
- Add `min_hold_candles` to Config
- Strategy cannot exit before N candles after entry
- Smoke test: with min_hold=6, no trade lasts fewer than 6 candles

### 3b. Signal smoothing / confirmation
- Require N consecutive bars of signal before entry
- Smoke test: trade count drops by >50% vs baseline

### 3c. Re-run hypothesis ledger
- Automated: backtest all 4 regimes -> parse output -> update ledger
- Smoke test: H008 friction/frequency correlation still holds
- Smoke test: net PnL improves with reduced frequency (or we learn why not)

---

## Phase 4: Ingestion Pipeline (2-3 commits)

### 4a. Data fetch script
- `scripts/fetch_data.sh` — pulls latest candles from Binance
- Appends to existing CSVs or creates new regime files
- Updates manifest.json with new hashes
- Smoke test: fetched data passes schema validation

### 4b. Pipeline script
- `scripts/pipeline.sh` — fetch -> validate -> backtest -> report
- Single command to update the whole system
- Smoke test: pipeline produces a dated report file

### 4c. Cron-ready
- Pipeline script is idempotent
- Writes to `out/reports/YYYY-MM-DD.json`
- Smoke test: running twice produces identical output

---

## Release Criteria for v0.2.0

All of these must pass:
1. `cargo test` — 0 failures
2. `scripts/smoke.sh` — all 12 smoke tests green
3. No dead code in the hot path (strategies.rs resolved one way or the other)
4. Epistemic server serves computed data
5. At least one strategy with positive net PnL in at least one regime
   after frequency loosening, OR honest documentation of why not
6. Pipeline script runs end-to-end without manual intervention

---

## Non-Goals for v0.2.0

- Live trading (Phase 5+)
- Engine/reducer convergence (Phase 6+)
- Multi-asset (Phase 7+)
- Blockchain integration (Phase 8+)
- Forecast engineering integration (Phase 9+)

First we prove we can run honest backtests on real data with automated
validation. Everything else is premature.
