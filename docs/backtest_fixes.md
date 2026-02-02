# Backtest Bugs / Fixes to Address

Based on baseline runs in `docs/baseline_backtests.md`.

## 1) Most strategies never trade
**Symptom:** Only `churn-0` had friction; others show zero PnL and zero friction.

**Likely causes:**
- Entry thresholds too high for dataset distribution.
- Z-score normalization too conservative or insufficient warmup.
- `edge_hurdle` gating blocks almost all trades.

**Fixes:**
- Log per-strategy `trade_count`, `hold_count`, and `skip_reason` to confirm gating.
- Add a `WARMUP_BARS` guard; only start after enough data for z-scores.
- Allow per-strategy overrides via env or config file (currently hardcoded in `build_churn_set`).

## 2) Event backtest detects no events
**Symptom:** `event_backtest` outputs only header.

**Likely causes:**
- Phase1 thresholds too high relative to dataset.
- Dataset lacks funding/OI/liquidation variation (zeros in CSV).

**Fixes:**
- Log counts per detector and thresholds used.
- Allow thresholds to be overridden via env with visible echo in output.
- Ensure CSV includes non-zero funding/OI/liquidation fields for event testing.

## 3) Build blocked by environment constraints
**Symptom:** `cargo run` fails due to crates.io access and cross-device link errors.

**Fixes:**
- Use `target/release/*` binaries for now.
- For rebuilds: ensure target dir is on same FS as source and dependencies are cached or vendored.

## 4) Friction attribution only for the orders that execute
**Symptom:** friction logged only for churn-0, nothing else.

**Fixes:**
- Add per-strategy diagnostics: `orders_submitted`, `orders_filled`, `orders_skipped`.
- Log slippage inputs (volume, vol, fill_ratio) for a few samples.

## 5) Baselines missing critical comparisons
**Fixes:**
- Add `baseline=buy_hold` and `baseline=no_trade` in all backtest modes.
- Add regime-conditioned baselines (low-vol/high-vol) to contextualize results.
