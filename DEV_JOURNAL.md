# Dev Journal

## 2026-01-31

### Focus
Harden the live loop and backtesting scaffolding so the system stops lying to itself under live conditions.

### Work Completed
- Added per-strategy, per-order unique IDs using a monotonic `order_seq`.
- Added a staleness guard that blocks trading when aux data is stale or incomplete.
- Fixed order sizing to use action qty (and close size for `Close`) instead of a fixed constant.
- Prevented fake fills in live mode by skipping stub execution; live mode now logs pending fills.
- WAL entries now include `strategy_id` for place/fill operations.

### Diagnostics
- `cargo test` failed due to cross-device link error when writing to `target/` (filesystem issue, not code).

### Risks / Gaps
- Live fill/reconciliation path still missing; live mode logs pending fills but does not ingest real fills.
- Aux data fetcher lacks caching/backoff and can silently go stale or rate-limit.
- Risk gating only checks funding/borrow completeness, not liquidation/depeg data.

### Next Actions
1) Implement a live fill poller + reconcile loop and wire it into the event bus.
2) Add aux caching/backoff with explicit staleness failures.
3) Tighten risk gate to require strategy-specific aux fields.
4) Re-run tests with `CARGO_TARGET_DIR=/tmp/arbitragefx-target` to avoid cross-device link failures.

## 2026-02-01

### Work Completed
- Added strategy-specific aux requirements and gated aux staleness only when required.
- Fixed `Close` order side selection (shorts close with BUY).
- Preserved previous aux state on fetch failures and logged errors instead of defaulting to zeros.
- Restored pending orders from WAL using `client_order_id` + `strategy_id`.
- Logged live fills into WAL with intent/strategy mapping.
- Removed live-path `continue` to ensure metrics update runs each loop.

### Backtest Baseline
- `cargo run --bin backtest data/btc_sim.csv` still shows large equity drawdowns despite small realized PnL; strategies appear to hold losing exposure until forced close.

### Risks / Gaps
- WAL `place_order` entries are mixed JSON; `WalEntry::PlaceOrder` doesn't include `client_order_id` or `strategy_id` (recovery relies on raw JSON entries).
- Strategy logic likely too permissive under this dataset; equity decay suggests position sizing / exit logic needs review.

### Next Actions
1) Add `client_order_id`/`strategy_id` to `WalEntry::PlaceOrder` for consistent recovery.
2) Rework strategy sizing and exits to avoid prolonged equity decay (evaluate `time_stop`, `edge_hurdle`, and vol gates).
3) Add a baseline report with equity curve slices for the churn set.

### Live-Readiness Patch (Reconcile + Cancel Policy)
- Added Binance reconcile client for open orders, spot balances, and futures positions.
- Wired fast reconcile loop (default 60s) and added cancel-after-N-candles policy.
- Live orders default to MARKET for safety; paper remains LIMIT.
- Strategy aux requirements now enforced (both strategies require full aux).

### Live Hardening Follow-up
- WAL recovery now prefers typed WAL entries; JSON fallback preserved.
- WAL place_order now carries strategy_id + client_order_id in typed entries.
- Added reconcile drift halt (default 2% or 0.0005 abs) to protect against state divergence.
- Cancel path now logs typed WAL cancel entry and applies CancelAck state.
