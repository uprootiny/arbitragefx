# Slippage Model Spec (Backtest)

## Goal
Capture execution slippage that scales with size and volatility.

## Inputs
- `qty` (order size)
- `volume` (bar volume / liquidity proxy)
- `volatility` (indicator snapshot)
- `SLIP_K` (base size coefficient)
- `VOL_SLIP_MULT` (volatility multiplier)

## Rules
- Size slippage grows with `qty / volume`.
- Volatility adds an additive slippage term.
- Total slippage is capped (5%) to prevent pathological spikes.

## Implementation
- `slippage_price(price, qty, liquidity, k, vol)` is used in backtest fills.
- Volatility increases slippage via `(1 + vol * 2.0)` multiplier.

## Invariants
- For fixed inputs, higher volatility → higher slippage.
- For fixed volatility, higher size → higher slippage.
