# ArbitrageFX — Development Guidelines

## Commit Discipline

Every commit must pass CI independently. The pipeline checks:
1. `cargo fmt --all -- --check` — zero formatting violations
2. `cargo clippy -- -D warnings` — zero clippy warnings
3. `cargo build --release` — clean release build
4. `cargo test` — all tests pass (currently 281)
5. Smoke tests — 28 end-to-end tests on real regime data
6. Coherence check — data schema and hash verification

**Atomic commits**: each commit should be a single coherent change. Don't mix formatting fixes with feature additions. Don't bundle unrelated changes.

**Commit messages**: imperative mood, 50-char subject. Body explains *why*, not *what*. End with `Co-Authored-By` if AI-assisted.

```
Add walk-forward validation with Bonferroni correction

Walk-forward with 4 windows and 70/30 train/test split reveals
0/12 strategies survive multiple comparison correction at alpha=0.05.
This strengthens H007 (no consistent alpha edge).

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

## Branching

- `main` is the production branch — always green
- Feature work can be done on `main` if commits are atomic and CI-validated
- Use feature branches for multi-day work or experimental changes

## Testing

### Test Categories
| Category | Location | Count | Purpose |
|----------|----------|-------|---------|
| Unit tests | `src/*.rs` (inline) | ~130 | Module-level correctness |
| Integration tests | `tests/*.rs` | ~120 | Cross-module behavior |
| Smoke tests | `tests/smoke.rs` | 28 | End-to-end on real data |
| Validation | `tests/backtest_validation.rs` | ~26 | Invariants and properties |

### When to Add Tests
- Every new public function should have at least one test
- Every bug fix should include a regression test
- Strategy decision branches should be tested individually
- Invariants (equity conservation, drawdown monotonicity) should be property tests

### Running Tests
```bash
cargo test                    # all tests
cargo test smoke              # smoke tests only
cargo test --test smoke       # smoke test file only
cargo test strategy_decision  # pattern match
```

## Guarantee Surface

The system tracks 18 backtest traps from the literature. Each trap has a guard status:

| Status | Meaning | Visual |
|--------|---------|--------|
| **Guarded** | Active guard with test coverage | Green |
| **Partial** | Guard exists but incomplete or uncalibrated | Yellow |
| **Unguarded** | No guard — known risk | Red |

**Goal**: expand the guarantee surface monotonically. Each point release should guard at least one new trap or strengthen a partial guard. Never regress.

Current score: **9/18 guarded**. Check with:
```bash
cargo run --release --bin workbench  # see trap checklist in dashboard
curl localhost:51723/api/health      # integrity_score field
```

## Hypothesis-Driven Development

Every strategy change should reference a hypothesis from `hypothesis_ledger.edn`:

```
H001: Momentum generates raw alpha         (stv 0.42 0.72) contested
H002: Friction dominates alpha              (stv 0.82 0.80) supported
H003: Position sizing limits DD <2%         (stv 0.95 0.84) established
...
```

**Before changing strategy logic**: identify which hypothesis you're testing. State the expected effect on truth values. Run the pipeline and update the ledger.

**After running backtests**: use `cargo run --bin update_ledger` to compute Bayesian updates. Review STV changes before applying.

## Code Conventions

### Rust
- `cargo fmt` before every commit
- `cargo clippy` with zero warnings
- Prefer `#[derive(Debug, Clone, Serialize)]` for all data types
- Use `Result<T, E>` for fallible operations, not panics
- No `unwrap()` in library code — only in binaries and tests
- Deterministic execution: use xorshift from timestamps, never `rand`

### Data
- CSV files: 11 columns (timestamp, OHLCV, funding, borrow, liq, depeg, OI)
- All datasets must pass `coherence_check` before use
- SHA256 hashes recorded in `hypothesis_ledger.edn`
- New datasets need regime classification via `classify_dataset()`

### Configuration
- All config via environment variables (55 parameters)
- Config hash (SHA256) for reproducibility — same hash = same result
- Document any new parameter in `state.rs::Config` with default and range

### Outputs
- Backtest results: `out/backtest/{date}.json`
- Walk-forward results: `out/walk_forward/report.json`
- Bench reports: `out/bench/report.json` + `out/bench/{date}.json`
- Evidence history: `out/ledger_history/updates.jsonl` (append-only)
- Dashboard: `docs/workbench.html` (self-contained, deployable by `cp`)

## Pipeline

The full pipeline runs with `./scripts/pipeline.sh`:

```
Step 1: Validate data     (coherence_check on all CSV files)
Step 2: Backtest           (4 regime datasets)
Step 3: Walk-forward       (train/test validation with correction)
Step 4: Update ledger      (Bayesian hypothesis updates)
Step 5: Bench              (performance profiling)
Step 6: Workbench          (generate dashboard HTML)
Step 7: Summary            (human-readable report)
```

Run the pipeline after any strategy or execution model change.

## Architecture Decisions

Document significant decisions in `DECISIONS.md` with:
- Date
- Context (what prompted the decision)
- Decision (what was chosen)
- Consequences (what this enables and constrains)
- Evidence (test results, benchmark data)

## Dependencies

**Zero new dependencies** is a strong preference. The current dependency set:
- `serde` + `serde_json` — serialization
- `sha2` — config hashing
- `chrono` — timestamps
- `tokio` + `reqwest` — async HTTP (live trading path only)

If you need a new dependency, justify it against the alternative of implementing the needed subset in ~100 lines.

## Self-Contained Artifacts

Dashboards and reports should be self-contained HTML files with embedded data. No external CDN links, no build tools, no node_modules. A file should be usable by opening it in a browser after `cp`.

This means:
- Inline CSS (no external stylesheets)
- Inline JS (no external scripts)
- Data embedded as JSON blob in a `<script>` tag
- Dark theme consistent with existing workbench

## Server Conventions

- Default port: high random port (51723), configurable via `PORT` env var
- CORS enabled (`Access-Control-Allow-Origin: *`)
- All API endpoints return `application/json`
- Health endpoint (`/api/health`) returns enriched status, not just `{"status":"ok"}`
- Expensive computations cached at startup, not per-request

## Release Process

1. Ensure CI green on `main`
2. Run full pipeline (`scripts/pipeline.sh`)
3. Verify workbench dashboard renders correctly
4. Update DEV_JOURNAL.md with session entry
5. Tag release: `git tag v0.X.Y`
6. Push tags: `git push --tags`
