# Partial Fill Model Spec (Backtest)

## Goal
Simulate partial fills for limit/realistic execution.

## Inputs
- `max_fill_ratio` (0.0–1.0)
- `qty` (order size)

## Rules
- Each eligible fill applies `qty * max_fill_ratio`.
- Remainder is re‑queued with updated `submit_ts`.
- Orders can take multiple bars to fill.

## Invariants
- Total filled quantity across partials equals original order size (within tolerance).
- Fill quantity per step never exceeds original order size.

## Notes
This is a coarse model until live fill data can calibrate.
