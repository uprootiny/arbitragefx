# Backtest-Realism Roadmap (≈ 52 tickets)

**Focus:** Instrumented backtests with latency/slippage/regime, real data handling, sweeps, correction.

## Sections

1. **Data & Aux Quality** (16)
   - B-001 [S] dataset manifest spec ✓  
   - B-002 [I] dataset manifest writer ✓  
   - B-003 [T] manifest includes dataset hash ✓  
   - B-004 [I] data schema validator ✓  
   - B-005 [T] schema rejects bad rows ✓  
   - B-006 [T] schema accepts good rows ✓  
   - B-007 [I] gap detector ✓  
   - B-008 [T] gap logged ✓  
   - B-009 [I] TTL enforcement ✓  
   - B-010 [T] marks stale aux ✓  
   - B-011 [I] aux cache/backoff  
   - B-012 [T] backoff increases  
   - B-013 [I] conversion tool for archives  
   - B-014 [T] converter handles 0 volume  
   - B-015 [T] converter handles NaN  
   - B-016 [S] data quality report spec

2. **Execution Realism** (18)
   - B-017 [S] latency model spec ✓  
   - B-018 [I] latency jitter implementation ✓  
   - B-019 [T] latency applied to orders ✓  
   - B-020 [S] slippage model spec ✓  
   - B-021 [I] vol-scaled slippage ✓  
   - B-022 [T] slippage rises with vol ✓  
   - B-023 [S] partial fill spec ✓  
   - B-024 [I] partial fills simulation ✓  
   - B-025 [T] partial fills accumulate ✓  
   - B-026 [S] fee model spec ✓  
   - B-027 [I] fee layer ✓  
   - B-028 [T] fees reduce PnL ✓  
   - B-029 [S] funding/borrow model spec  
   - B-030 [I] carry adjustment  
   - B-031 [T] funding affects carry strategy  
   - B-032 [T] borrow influences w/ sign  
   - B-033 [T] execution parity paper/live  
   - B-034 [T] fill dedupe by ID

3. **Strategy & Metrics** (18)
   - B-035 [S] regime segmentation spec  
   - B-036 [I] regime classifier  
   - B-037 [T] regime output used  
   - B-038 [S] time-of-day spec  
   - B-039 [I] hour stratifier  
   - B-040 [T] stratification logs  
   - B-041 [S] walk-forward spec  
   - B-042 [I] walk-forward harness  
   - B-043 [T] walk-forward boundaries  
   - B-044 [S] sweep spec  
   - B-045 [I] sweep runner  
   - B-046 [T] sweep determinism  
   - B-047 [T] multiple testing flag  
   - B-048 [T] untrustworthy labeling  
   - B-049 [T] reality score stable  
   - B-050 [T] summary per regime  
   - B-051 [T] baseline report output format  
   - B-052 [I] reality score metric
