# Load-Bearing Skeleton

This is a terse, rigorous reconstruction of the *minimum* system required to
run a deterministic, auditable trading loop. Everything else is optional.

## Core invariants

1. **Single mutation site**: all state changes happen in the engine loop.
2. **WAL-before-state**: order intents and fills are written before applying.
3. **Risk only blocks**: risk can halt or close, never force new risk.

## Files

- `src/skeleton/state.rs` — state models and invariants.
- `src/skeleton/strategy.rs` — strategy interface (pure, side-effect-free).
- `src/skeleton/exec.rs` — execution adapter interface + paper impl.
- `src/skeleton/log.rs` — JSONL event log + run metadata.
- `src/skeleton/wal.rs` — append-only intent/fill log.
- `src/skeleton/engine.rs` — single loop: ingest → decide → guard → exec → apply → log.
- `src/bin/skeleton_loop.rs` — runnable example loop.

## Runtime loop (minimal)

```
loop:
  candle = fetch_market()
  indicators.update(candle)
  action = strategy.decide(state)
  action = risk.guard(action)
  fill = exec.simulate_or_place(action)
  wal.append(intent/fill)
  state.apply(fill)
  log.append(snapshot)
  sleep_until_next_candle()
```

## What is intentionally excluded

- Multi-strategy orchestration
- Reconciliation and drift correction
- Exchange-specific retry/WS glue
- Complex metrics and summaries
- External dashboards

Those can be layered on later without changing the core invariants above.
