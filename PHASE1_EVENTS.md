# Phase 1 Event Detection (Engineering Spec)

All tests are event-conditioned. These are the **Phase 1** event detectors.

## Funding Imbalance (Reversion)
Signal:
- `|funding_rate| >= funding_p95 * FUNDING_PCTL_MULT`
- `oi_change > OI_SPIKE_TH`

Output:
`FundingImbalance` event

## Liquidation Cascade (Momentum)
Signal:
- `|price_velocity| > VEL_MULT * vol_ratio`
- `liquidation_score > LIQ_SCORE_TH`

Output:
`LiquidationCascade` event

## Stablecoin Depeg (Snapback)
Signal:
- `|stable_depeg| > DEPEG_TH`

Output:
`StablecoinDepeg` event

## Config (env)
- `FUNDING_PCTL_MULT` (default 1.0)
- `OI_SPIKE_TH` (default 0.08)
- `VEL_MULT` (default 3.0)
- `LIQ_SCORE_TH` (default 3.0)
- `DEPEG_TH` (default 0.005)
