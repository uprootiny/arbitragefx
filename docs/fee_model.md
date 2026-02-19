# Fee Model Spec (Backtest)

## Goal
Ensure fees are applied to every fill and reduce net PnL.

## Inputs
- `FEE_RATE` (per‑notional fee)
- `price`, `qty`

## Rules
- Fee = `price * |qty| * fee_rate`.
- Fees subtract from cash on every fill.

## Invariants
- Higher fee rate → lower net PnL (all else equal).
