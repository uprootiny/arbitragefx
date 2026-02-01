# Engineering Review: arbitragefx

**Date**: 2026-02-01
**Commits reviewed**: 630e696..e1411bf
**Total LOC**: 12,432
**Tests**: 77 passing

---

## Executive Summary

The codebase implements an ethically-constrained cryptocurrency trading system with:
- Event-driven architecture with deterministic replay
- Buddhist ethics framework (Three Poisons guards, Eightfold Path checklist)
- Live execution via Binance WebSocket + polling fallback
- WAL-based crash recovery with per-strategy snapshots

**Overall assessment**: Solid foundation with several structural issues to address.

---

## Architecture

### Strengths

| Component | Assessment |
|-----------|------------|
| Event bus | Clean, typed, deterministic ordering |
| Reducer | Pure function, state hashing for replay |
| Ethics guards | Well-formalized, testable constraints |
| Live fill path | WebSocket primary, poll fallback, proper ordering |
| WAL recovery | Per-strategy snapshots, fill replay |

### Structural Issues

| Issue | Severity | Details |
|-------|----------|---------|
| **Duplicate types** | High | Two `StrategyState` (strategy.rs:104, engine/state.rs:534), two `MarketView` |
| **Dead code** | Medium | `cancel` TODOs in engine_loop.rs, unused retry helpers |
| **Module coupling** | Medium | main.rs at 570 LOC, mixes execution + strategy + logging |

### Recommended Refactoring

1. **Consolidate types**: Single `StrategyState`, single `MarketView`
2. **Extract execution module**: Move order placement/fill handling to `execution.rs`
3. **Add Exchange::cancel**: Complete the trait or remove TODOs

---

## Test Coverage

### Well-Tested (>4 tests)

| Module | Tests | Notes |
|--------|-------|-------|
| engine/reducer | 10 | Core trading logic |
| strategy | 8 | MarketAux, requirements |
| risk | 8 | MTM, exposure, cooldown |
| engine/policy | 7 | Intent/gate logic |
| feed/aux_data | 6 | Caching, backoff |

### Untested (0 tests, high risk)

| Module | LOC | Risk |
|--------|-----|------|
| state.rs | 588 | **Critical** - Core state machine, strategies |
| backtest.rs | 281 | **Critical** - Validation path |
| engine/state.rs | 540 | **High** - Engine types |
| exchange/binance.rs | 310 | **High** - Live execution |
| feed/binance_live.rs | 220 | **High** - Live fills |
| reliability/circuit.rs | ~50 | **Medium** - Failure handling |

### Coverage Actions

```
Priority 1: state.rs (core strategies), backtest.rs (validation)
Priority 2: exchange/binance.rs, binance_live.rs (live path)
Priority 3: circuit.rs, storage.rs (reliability)
```

---

## Correctness Analysis

### Division Safety ✓

All 20 division operations are guarded with `.max(1.0)`, `if x > 0.0`, or equivalent.

### Position Handling

`apply_fill` in strategy.rs:134-170 correctly handles:
- Closing positions (realized PnL calculation)
- Increasing positions (weighted average entry)
- Position flips (new entry price)

### Risk Gate ✓

risk.rs:200-254 properly allows:
- `Close` even when halted/cooldown
- Risk-reducing sells when over trade limit
- MTM unrealized loss in daily limit check

### Potential Issues

| Issue | Location | Risk |
|-------|----------|------|
| No bounds on fill channel | main.rs:122 (256 cap) | Low - could block sender |
| Timestamp overflow | Various `/ 86_400` | Very Low - 2^64 secs |
| Silent aux defaults | MarketAux::default() | Fixed - now has_* flags |

---

## Security Considerations

### API Key Handling ✓

- Keys from env vars, not hardcoded
- Signature via HMAC-SHA256 (signing.rs)
- No key logging observed

### Input Validation

| Input | Validated? | Notes |
|-------|------------|-------|
| CSV rows | Partial | Column count checked, no range validation |
| WebSocket messages | Yes | Parsed as typed structs |
| Fill quantities | Yes | Checked > 0.0 before processing |

### Recommendations

1. Add rate limiting to aux fetcher (partially done with backoff)
2. Validate fill prices against order prices (slippage check)
3. Add maximum position size per symbol

---

## Performance Observations

### Profiling Support ✓

`ProfileScope` in logging.rs with `PROFILE_SAMPLE` env for production sampling.

### Potential Bottlenecks

| Area | Observation |
|------|-------------|
| Aux fetch | Parallel tokio::join, cached with TTL |
| Fill processing | Single-threaded in main loop |
| State hashing | SHA256 on every reduce (could batch) |

---

## Remaining TODOs

```rust
src/bin/engine_loop.rs:172: // TODO: implement cancel via adapter
src/bin/engine_loop.rs:177: // TODO: implement cancel all
```

---

## Action Items

### Immediate (before production)

1. [ ] Add tests for state.rs (SimpleMomentum, CarryOpportunistic)
2. [ ] Add tests for backtest.rs (end-to-end simulation)
3. [ ] Consolidate duplicate StrategyState types
4. [ ] Add fill price slippage validation

### Short-term

5. [ ] Extract execution module from main.rs
6. [ ] Add Exchange::cancel or remove dead TODOs
7. [ ] Add integration test: crash → WAL replay → hash match
8. [ ] Add circuit breaker tests

### Medium-term

9. [ ] Wire drift_tracker into live loop
10. [ ] Add strategy registry with required aux fields
11. [ ] Add systemd service templates

---

## 24 Engineering Snags

### Critical (can lose money)

| # | Issue | Status | Details |
|---|-------|--------|---------|
| 1 | **WAL recovery overwrote strategies** | ✅ Fixed | Single `last_snapshot` meant last strategy's snapshot applied to all. Now `snapshots_by_strategy` HashMap. |
| 2 | **Backtest pending orders shared** | ✅ Fixed | All strategies filled each other's orders. Added `strategy_idx` on PendingOrder. |
| 3 | **Risk engine ignored unrealized loss** | ✅ Fixed | Only checked realized PnL. Added MTM calculation with current price. |
| 4 | **Close blocked by halt/cooldown** | ✅ Fixed | Couldn't unwind positions when halted. Now allows Close through all guards. |
| 5 | **Trading on missing aux data** | ✅ Fixed | `funding_rate=0.0` treated as signal. Added `has_funding` flags. |
| 6 | **Client ID collisions** | ✅ Fixed | `CID-{ts}` reused for concurrent orders. Now `CID-{strategy}-{ts}-{seq}`. |

### High (wrong results)

| # | Issue | Status | Details |
|---|-------|--------|---------|
| 7 | **Entry price always overwritten** | ✅ Fixed | Every fill replaced entry_price. Now weighted average. |
| 8 | **Backtest left positions open** | ✅ Fixed | No forced close at end. Added force close at last bar. |
| 9 | **Exposure check division by zero** | ✅ Fixed | Divided by `equity.abs()` when zero. Added `.max(1.0)`. |
| 10 | **Aux defaults silently zero** | ✅ Fixed | depeg=0.0 meant "no depeg" not "unknown". Added `has_depeg`. |
| 11 | **Strategies used liq score unchecked** | ✅ Fixed | Zero score = "no data". Added `has_liquidations` check. |

### Medium (brittle behavior)

| # | Issue | Status | Details |
|---|-------|--------|---------|
| 12 | **Two `StrategyState` types** | ⚠️ Open | strategy.rs:104 vs engine/state.rs:534 |
| 13 | **Two `MarketView` types** | ⚠️ Open | Same issue, different fields |
| 14 | **Cancel TODOs with no Exchange support** | ⚠️ Open | Dead code that looks active |
| 15 | **46 unwrap() calls** | ⚠️ Open | Scattered panic points in non-test code |
| 16 | **main.rs at 570 LOC** | ⚠️ Open | Mixes execution + strategy + logging |
| 17 | **No tests for state.rs (588 LOC)** | ⚠️ Open | Core strategies completely untested |
| 18 | **No tests for backtest.rs** | ⚠️ Open | Validation path untested |
| 19 | **ProfileScope uses rand::random()** | ⚠️ Open | Adds dependency for sampling |
| 20 | **Fill channel bounded at 256** | ⚠️ Open | Could block sender under burst |

### Low (code quality)

| # | Issue | Status | Details |
|---|-------|--------|---------|
| 21 | **Retry helpers unused** | ⚠️ Open | `is_retryable_http_error` never called |
| 22 | **Order SM variants unused** | ⚠️ Open | `CancelRequest`, `Reject`, `Timeout` never constructed |
| 23 | **`in_profit` variable unused** | ⚠️ Open | policy.rs:436 |
| 24 | **Inconsistent error handling** | ⚠️ Open | Mix of `?`, `unwrap_or_default()`, panic |

---

## Verdict

**Ready for paper trading**: Yes
**Ready for live (small size)**: Yes, with monitoring
**Ready for production**: No - needs state.rs tests, backtest.rs tests, type consolidation

The ethical framework (Three Poisons, Eightfold Path) is well-implemented and provides meaningful constraints. The live fill reconciliation path is solid. Main gaps are test coverage for core components and structural cleanup.

**Fixed this session**: 11 critical/high issues
**Remaining**: 13 medium/low issues
