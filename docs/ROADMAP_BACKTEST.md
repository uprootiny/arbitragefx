# Backtest-Realism Roadmap (52 tickets)

**Focus:** Instrumented backtests with latency/slippage/regime, real data handling, sweeps, correction.
**Updated:** 2026-02-19 (30/52 done)

## Progress: 30/52 tickets complete

## Sections

1. **Data & Aux Quality** (16) — 12 done
   - B-001 [S] dataset manifest spec ✓
   - B-002 [I] dataset manifest writer ✓ (src/bin/dataset_manifest.rs)
   - B-003 [T] manifest includes dataset hash ✓ (SHA256 in data module)
   - B-004 [I] data schema validator ✓ (src/data/mod.rs validate_schema)
   - B-005 [T] schema rejects bad rows ✓ (tests/data_quality.rs)
   - B-006 [T] schema accepts good rows ✓ (tests/data_quality.rs)
   - B-007 [I] gap detector ✓ (src/data/mod.rs analyze_csv)
   - B-008 [T] gap logged ✓ (tests/data_quality.rs detects_gaps_and_staleness)
   - B-009 [I] TTL enforcement ✓
   - B-010 [T] marks stale aux ✓
   - B-011 [I] aux cache/backoff ✓ (feed/aux_data.rs)
   - B-012 [T] backoff increases ✓
   - B-013 [I] conversion tool for archives
   - B-014 [T] converter handles 0 volume
   - B-015 [T] converter handles NaN
   - B-016 [S] data quality report spec

2. **Execution Realism** (18) — 12 done
   - B-017 [S] latency model spec ✓ (docs/latency_model.md)
   - B-018 [I] latency jitter implementation ✓ (backtest.rs latency_delay)
   - B-019 [T] latency applied to orders ✓
   - B-020 [S] slippage model spec ✓ (docs/slippage_model.md)
   - B-021 [I] vol-scaled slippage ✓ (ExecConfig::realistic)
   - B-022 [T] slippage rises with vol ✓
   - B-023 [S] partial fill spec ✓ (docs/partial_fill_model.md)
   - B-024 [I] partial fills simulation ✓
   - B-025 [T] partial fills accumulate ✓
   - B-026 [S] fee model spec ✓ (docs/fee_model.md)
   - B-027 [I] fee layer ✓ (friction accounting in backtest.rs)
   - B-028 [T] fees reduce PnL ✓ (smoke test S11)
   - B-029 [S] funding/borrow model spec — **blocked**: need real funding data in backtest
   - B-030 [I] carry adjustment — partial (CarryOpportunistic reads aux, but aux is zeros in CSV backtest)
   - B-031 [T] funding affects carry strategy
   - B-032 [T] borrow influences w/ sign
   - B-033 [T] execution parity paper/live — **blocked**: no live trading yet
   - B-034 [T] fill dedupe by ID

3. **Strategy & Metrics** (18) — 6 done
   - B-035 [S] regime segmentation spec ✓ (4 regimes in hypothesis_ledger.edn)
   - B-036 [I] regime classifier — manual (file-based, no automated detection)
   - B-037 [T] regime output used ✓ (per-regime backtest in pipeline.sh)
   - B-038 [S] time-of-day spec
   - B-039 [I] hour stratifier
   - B-040 [T] stratification logs
   - B-041 [S] walk-forward spec
   - B-042 [I] walk-forward harness
   - B-043 [T] walk-forward boundaries
   - B-044 [S] sweep spec ✓ (12-variant churn sweep documented)
   - B-045 [I] sweep runner ✓ (build_churn_set in state.rs)
   - B-046 [T] sweep determinism ✓ (smoke test S07)
   - B-047 [T] multiple testing flag — **needed**: 60 comparisons without correction
   - B-048 [T] untrustworthy labeling
   - B-049 [T] reality score stable
   - B-050 [T] summary per regime ✓ (pipeline.sh produces per-regime output)
   - B-051 [T] baseline report output format
   - B-052 [I] reality score metric

## Priority Queue (next tickets to tackle)

1. **B-047** Multiple testing correction — most impactful for epistemic honesty
2. **B-041/042/043** Walk-forward — most impactful for strategy validation
3. **B-036** Automated regime classifier — enables reconnaissance use case
4. **B-029/030** Funding model in backtest — enables carry strategy evaluation
5. **B-016** Data quality report spec — enables pipeline quality gates
