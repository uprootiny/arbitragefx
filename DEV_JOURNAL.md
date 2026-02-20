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

## 2026-02-19

### Focus
Validation grounding: walk-forward analysis, hypothesis Bayesian updates, config serialization, formal invariant tests, dead code resolution, multi-regime validation runs.

### Work Completed
- Config serialization: SHA256 config hash for reproducible runs, `BacktestResult.to_json()` with config_hash and candle_count
- Walk-forward validation: `src/walk_forward.rs` with train/test splits, Bonferroni correction, p-values per strategy. Result: 0/12 strategies survive correction
- Automated hypothesis updates: `src/bin/update_ledger.rs` reads BacktestResult or WalkForwardResult, computes Bayesian STV updates for H001-H009
- Formal invariant tests for `apply_fill()`: equity conservation, drawdown monotonicity, fill accounting, PnL sign consistency
- Dead code resolution: archived 4 spin-off projects (hypothesis-lab, mindful-engine, indicators-lib, research-scanner), wired 3 modules into active pipeline (narrative_detector, drift_tracker, backtest_traps)
- Multi-regime validation: ran backtest + walk-forward across btc_real_1h, btc_bull_1h, btc_range_1h, btc_bear2_1h
- 8 new strategy decision tests for untested SimpleMomentum branches
- GitHub Pages workflow with styled hypothesis dashboard

### Findings
- Walk-forward: 0/12 strategies survive Bonferroni correction across 4 windows
- Overfit ratio (test/train): mean 9.08 — extreme overfitting detected
- H003 (drawdown bounded <2%): strengthened to (stv 0.95 0.84)
- H007 (no consistent alpha): strengthened to (stv 0.78 0.80)
- Capital preservation confirmed as the system's actual edge

### Commits
- `4917ee0` Resolve dead code: archive 4 spin-off projects, wire 3 modules
- `cef7b81` Add 8 strategy decision tests for untested SimpleMomentum branches
- `5ca5c1f` Add GitHub Pages workflow with styled hypothesis dashboard
- `1efbafc` Add min_hold_candles to reduce overtrading (H008 response)
- `8bb2a70` Add README with quick start, findings, project structure
- `b8fe76e` Rewrite ARCHITECTURE.md with system diagram, module map, evidence

## 2026-02-20

### Focus
Phase 1 workbench affordances: build the observation surface. Smoke tests, bench profiler, workbench dashboard, pipeline integration, CI/CD green, live deployment.

### Session Arc
1. Compiled and ran 28 smoke tests (S19-S30) — all pass
2. Built bench profiler binary: ~50k candles/sec, <3MB RSS, 4 datasets in 396ms
3. Built workbench dashboard generator: self-contained HTML with embedded JSON
4. Wired bench + workbench into pipeline.sh and GitHub Pages
5. Committed in 4 atomic commits, each CI-validated before the next
6. Fixed cargo fmt violation across 63 files
7. Created GitHub repo, pushed, got CI green

### Phase 1 Workbench Enrichment (6/6 complete)
- **R1**: JSONL history append in update_ledger (`out/ledger_history/updates.jsonl`) + evidence timeline sparklines per hypothesis in workbench
- **A2**: `trap_status()` + `integrity_score()` in backtest_traps.rs — 18 traps with Guarded/Partial/Unguarded classification. Score: 9/18
- **O1**: Enriched `/api/health` — test count (168), integrity score (9/18), dataset inventory (16 datasets with row counts), last pipeline date, invariant status
- **R4**: Uncertainty map kanban board — 5 columns (well-established → untested) from hypothesis_ledger.edn
- **R3**: Regime-conditioned strategy leaderboard — ranked by equity PnL with DD<2% safety badges
- **O2**: Resource trend charts — throughput + timing SVG from bench_history

### Server Enhancement
- Epistemic server now serves workbench dashboard at `GET /`
- Pre-loads workbench HTML at startup (no per-request file I/O)
- Fast test count via `#[test]` attribute scanning (not cargo invocation)
- Running on port 51723

### Documentation
- `SYSTEM_DESIGN.md`: complete as-is topology (21 binaries, 19 modules, guarantee surface, CI/CD, ecosystem)
- `DESIGN_V2.md`: v2.0 proposal — 3-layer architecture (CLJS observation, Rust service, Rust computation), dead code resolution, config files, ecosystem integration
- `REFLECTION_SESSION.md`: lessons for CLAUDE.md (CI-first commits, observation surfaces, keystone changes)

### Deployments
- GitHub Pages: https://uprootiny.github.io/arbitragefx/ (workbench dashboard)
- Local server: http://localhost:51723/ (dashboard + API)
- Repo: https://github.com/uprootiny/arbitragefx (public)

### Commits
- `72ce2c4` Add 12 smoke tests (S19-S30)
- `acbf81c` Add bench binary for profiling
- `9cad52b` Add workbench dashboard generator
- `2a85ee4` Wire bench and workbench into pipeline and GitHub Pages
- `c5f9512` Run cargo fmt across entire codebase
- `78cd1cc` Add Phase 1 workbench affordances: timeline, kanban, traps, leaderboard
- `3b1311f` Fix Pages workflow
- `a83abaf` Serve workbench dashboard at server root
- `6e63af9` Add system design, v2.0 sketch, and session reflection docs

### Risks / Gaps
- 5/18 traps still unguarded (in-sample confusion, curve fitting, backtest-to-live gap, multiple comparisons alpha, tail risk)
- ~2,000 lines of dead code (strategies.rs, signals.rs, filters.rs) — planned for v2.0-alpha archive
- Config still via env vars only — TOML config planned for v2.0
- Narrative detector thresholds uncalibrated
- No CLJS observation layer yet — proposed for v2.0-beta

### Next Actions
1. Archive dead code (strategies.rs, signals.rs, filters.rs) → v2.0-alpha
2. Add `/api/traps`, `/api/graph`, `/api/timeline/{id}` endpoints
3. Scaffold CLJS observation layer (shadow-cljs + Reagent)
4. Write CONTRIBUTING.md with development guidelines
