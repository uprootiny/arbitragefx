# Latency Model Spec (Backtest)

## Goal
Simulate realistic execution latency with deterministic jitter so replay is stable.

## Inputs
- `LAT_MIN` / `LAT_MAX` (seconds)
- `submit_ts` (order submit timestamp)
- `strategy_idx` (deterministic per strategy)

## Rules
- If `LAT_MAX <= LAT_MIN`, delay = `LAT_MIN`.
- Otherwise delay is a bounded jitter in `[LAT_MIN, LAT_MAX]`.
- Jitter must be deterministic (no RNG dependency) for reproducibility.

## Implementation
- Use a simple xorshift on `submit_ts` + `strategy_idx`.
- Apply delay before fills in backtest.

## Invariants
- Delay always within bounds.
- Same inputs → same delay.

## Notes
This is a lightweight model; upgrade to calibrated distribution once live data is available.
