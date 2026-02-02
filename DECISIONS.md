# Decisions

## 2026-01-31
- Language: Rust for the strategy API and state layout.
- Added `src/strategy.rs` with `Candle`, `IndicatorSnapshot`, `MarketView`, `PortfolioState`,
  `StrategyState`, `MetricsState`, `Action`, and `Strategy` trait.
- Operational goal captured: single-process, always-on trading loop with REST polling,
  systemd supervision, SQLite persistence, and a hard risk guard.
- Trading loop constraints: candle-close polling only, no websockets, no replay engine,
  one process with data poller/strategy/execution/risk/state store.
- Risk guard defaults: max position 5% capital, max daily loss 2%, one trade at a time,
  10-minute cooldown after loss, kill switch on API error spike, `/tmp/STOP` kill file.
- Persistence: SQLite tables for balances, positions, trades, metrics; commit after each loop.
- Reliability: systemd `Restart=always`, stdout + file logs; retry once on API timeout,
  wait 30s on network error, halt trading if exchange down.
- Horizon focus: 5 min–6 hr window (momentum + vol clustering + funding/cross-venue lag).
- Strategy signal plan: O(1) features (EMA fast/slow, rolling vol, volume shock, VWAP dev)
  with linear/rule-based score and thresholding; avoid L2 order book prediction.
- Execution guard: require expected edge > fees + slippage (~0.2–0.4% hurdle).
- Cycle timing: 60s market poll, decision per poll, metrics persist every 5 min.
- Starting target: Binance or Kraken, BTC/USDT or ETH/USDT, small capital.
- Implementation default: Binance public REST for candles; execution adapter is stubbed
  (no signed orders) until exchange + pair are explicitly chosen.
- Parameterization: added `PARAMETERS.md` with concrete defaults and ranges for
  EMA windows, thresholds, weights, edge hurdle, and risk controls.
- Formal verification scaffold: added pure order state machine + invariants
  (`src/verify/*`) and a minimal TLA+ spec in `docs/formal/order_sm.tla`.
- Added fuzz generator (`src/bin/fuzz.rs`) and a minimal ledger TLA+ spec
  (`docs/formal/ledger.tla`) to stress/reason about portfolio consistency.
- Replay harness now asserts equity consistency; added a tiny permutation tool
  (`src/bin/permute.rs`) to stress ordering assumptions.
- Viable strategy set narrowed to: funding carry, liquidation cascade momentum,
  stablecoin depeg snapbacks, borrow vs funding spread, volatility regime switching.
- Implemented ground-level trigger logic and thresholds in code, documented in
  `TRIGGERS.md`, and wired into the default strategy.
- Added CSV-based backtest harness (`src/backtest.rs`, `src/bin/backtest.rs`)
  for API-only evaluation of the trigger set.
- Backtest execution physics now include latency, partial fills, slippage, and fees.
- Added formal data schema (`DATA_SCHEMA.md`) and a feature pipeline module
  (`src/features.rs`) with rolling funding percentiles, OI change, and vol ratios.
- Added Phase 1 event detection algorithms and config (`src/events.rs`,
  `PHASE1_EVENTS.md`) for funding imbalance, liquidation cascades, and depegs.
- Added exchange-agnostic event backtest (`src/bin/event_backtest.rs`) for
  event-conditioned PnL with latency/slippage/fees.
- Added minimal live loop and carry+opportunistic strategy set
  (`src/bin/live_loop.rs`, `build_carry_event_set`).
- Backtest now runs a 12-strategy churn set and prints per-strategy friction
  (fees + slippage) alongside PnL.
- Added baselines to backtest output (buy-and-hold, no-trade, friction-only).
- Added public-data fetch + CSV build scripts for Binance 1m candles
  (`scripts/fetch_binance_klines.sh`, `scripts/build_csv_from_binance.py`).
- Added reliability middleware scaffolding (WAL, order state, circuit breaker),
  a unified adapter trait, capital pressure feed events, and fault injection stubs.
- Wired reliability scaffolding into the runtime loop with structured logs for
  STRATEGY/RISK/EXEC/FILL/POSITION/METRICS/POLLER, plus WAL writes.
- Replaced plain logs with JSON shovel-layer logs (exec wrapper, order_state,
  reconcile, wal, risk_guard, circuit_breaker, audit, position_agg, flow_feed).
