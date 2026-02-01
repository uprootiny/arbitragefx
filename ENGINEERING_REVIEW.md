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

## Verdict

**Ready for paper trading**: Yes
**Ready for live (small size)**: Yes, with monitoring
**Ready for production**: No - needs state.rs tests, backtest.rs tests, type consolidation

The ethical framework (Three Poisons, Eightfold Path) is well-implemented and provides meaningful constraints. The live fill reconciliation path is solid. Main gaps are test coverage for core components and structural cleanup.
