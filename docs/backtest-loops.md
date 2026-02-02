# Backtesting Loops of Reasoning & Feedback

## Loop 1 — Hypothesis → Test → Evidence → Decision
1. Hypothesis written in ledger.
2. Test specified with data window + metrics.
3. Evidence artifacts linked.
4. Decision recorded (refine, keep, drop).

## Loop 2 — Failure → Diagnosis → Guardrail
1. Failure or anomaly detected.
2. Root cause classification (data, model, execution).
3. Guardrail added and enforced in CI/policy.

## Loop 3 — Regime Shift → Adaptation
1. Detector flags regime shift.
2. Backtest segmentation updated.
3. Model or rule set swapped.

## Practical Requirements
- Every test must create an artifact in `out/` or `docs/`.
- Every hypothesis must link at least one evidence entry.
- No promotion to "supported" without reproducible artifacts.
