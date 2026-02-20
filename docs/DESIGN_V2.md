# ArbitrageFX v2.0 — Design Sketch

**Date:** 2026-02-20
**Status:** Proposal, not approved
**Predecessor:** DESIGN.md v3 (2026-02-18), BLUEPRINT.md, CRITIQUE.md

---

## How We Got Here

### The Process

v1.0 started as an imperative backtest loop with hardcoded strategies. The system grew organically through a recognizable pattern:

1. **Foundation** — CSV ingestion, strategy trait, backtest loop, risk guards
2. **Reckoning** — CRITIQUE.md exposed ~4,000 lines of dead code, two divergent architectures, uncalibrated parameters
3. **Grounding** — Real data (5 regimes), honest friction accounting, hypothesis ledger with Bayesian truth values
4. **Observation** — Workbench dashboard, epistemic server, trap checklist, evidence timeline
5. **Validation** — Walk-forward with Bonferroni correction, 281 tests, CI pipeline, smoke tests

Each phase was driven by confronting what the system actually did versus what it claimed to do. The CRITIQUE.md was the turning point: it named the gap between aspiration and evidence, and every subsequent commit reduced that gap.

### What v1.x Proved

**Established findings** (high strength, high confidence):
- H003: Position sizing limits drawdown to <2% — the system preserves capital
- H008: Trade frequency is the primary friction determinant — fewer trades = less friction
- H009: The system's edge is capital preservation, not alpha generation

**Negative findings** (also valuable):
- H001: Raw alpha is contested — momentum doesn't reliably generate returns after friction
- H007: No strategy consistently beats no-trade across all regimes
- Walk-forward: 0/12 strategies survive Bonferroni correction

**Meta-finding:** The hypothesis-driven approach works. The system knows what it doesn't know. The uncertainty map is honest. This epistemic infrastructure is the actual product.

### What v1.x Left Unresolved (from CRITIQUE.md)

| Issue | Status | v2 Plan |
|-------|--------|---------|
| Two architectures, zero convergence | Open | Archive engine/, promote backtest loop |
| strategies.rs dead code (808 lines) | Open | Delete or benchmark against SimpleMomentum |
| signals.rs/filters.rs dead code | Open | Delete with strategies.rs |
| Config: 55 env vars, zero snapshots | Open | Config file with hash-addressable snapshots |
| "Realistic" execution uncalibrated | Open | Requires live trading data — deferred |
| Narrative detector has no evidence | Open | Test against regime classification accuracy |

---

## v2.0 Vision

**Shift:** From "build a trading system" to "build an epistemic research workbench that happens to backtest strategies."

The realization: the most valuable output of v1 is not any trading strategy — it's the hypothesis ledger, the observation surface, the uncertainty map, and the guarantee surface expansion process. v2 doubles down on this.

### Architecture: Three Layers

```
┌─────────────────────────────────────────────────────────┐
│                   OBSERVATION LAYER                      │
│                                                         │
│  ClojureScript browser-side (Reagent)                   │
│  ├── Ontology graph (force-directed, d3/vanilla)        │
│  ├── Hypothesis timeline (sparklines, STV history)      │
│  ├── Trap checklist (live guard status)                 │
│  ├── Session workspace (domain packs)                   │
│  └── Reconciliation dashboard                           │
│                                                         │
│  Polls /api/* endpoints, falls back to embedded JSON    │
│  Deployed as static CLJS build to GitHub Pages          │
└────────────────────────┬────────────────────────────────┘
                         │ HTTP JSON
┌────────────────────────▼────────────────────────────────┐
│                    SERVICE LAYER                         │
│                                                         │
│  Rust epistemic_server (port 51723)                     │
│  ├── GET /api/state     — full epistemic state          │
│  ├── GET /api/health    — enriched health               │
│  ├── GET /api/graph     — relationship graph JSON       │
│  ├── GET /api/traps     — trap status array             │
│  ├── GET /api/timeline/{id} — STV history per hypothesis│
│  ├── GET /api/runs      — activity feed                 │
│  ├── GET /api/sessions  — session template list         │
│  └── POST /api/run      — trigger backtest run          │
│                                                         │
│  Reads from out/ artifacts + hypothesis_ledger.edn      │
│  CORS enabled, stateless (reads filesystem per request) │
└────────────────────────┬────────────────────────────────┘
                         │ filesystem
┌────────────────────────▼────────────────────────────────┐
│                   COMPUTATION LAYER                      │
│                                                         │
│  Rust binaries (pipeline-orchestrated)                   │
│  ├── backtest       — strategy evaluation               │
│  ├── walk_forward   — out-of-sample validation          │
│  ├── update_ledger  — Bayesian hypothesis updates       │
│  ├── bench          — performance profiling             │
│  ├── reconcile      — integrity + gap analysis          │
│  ├── session        — structured experiment runner      │
│  └── reproduce      — deterministic replay verifier     │
│                                                         │
│  Writes to out/ as JSON/JSONL artifacts                 │
│  Pipeline: scripts/pipeline.sh orchestrates sequence    │
└─────────────────────────────────────────────────────────┘
```

---

## v2.0 Components

### 1. ClojureScript Observation Layer

**Rationale:** The user's ecosystem already uses ClojureScript (theirtents, plasmidia). Reagent provides reactive rendering. The observation layer should be a proper CLJS application, not embedded JS in a Rust string constant.

```
ui/
├── deps.edn
├── src/
│   └── arbitragefx/
│       ├── core.cljs        — app entry, routing, state atom
│       ├── api.cljs         — fetch /api/* with fallback to embedded JSON
│       ├── graph.cljs       — force-directed ontology graph
│       ├── timeline.cljs    — STV sparklines and history
│       ├── kanban.cljs      — uncertainty map board
│       ├── traps.cljs       — trap checklist component
│       ├── session.cljs     — session workspace
│       └── reconcile.cljs   — reconciliation dashboard
├── resources/public/
│   └── index.html
└── shadow-cljs.edn
```

**Key decisions:**
- shadow-cljs for compilation (standard in the ecosystem)
- Reagent atoms for state (no Redux-like complexity)
- Fetches from server when available, falls back to `__DATA__` global
- Single-page app with client-side routing
- Dark theme consistent with existing workbench

### 2. Ontology Graph (R2)

Visualize hypothesis-strategy-regime-dataset relationships as a force-directed graph.

```clojure
;; Node types
{:hypothesis {:shape :circle  :color "#58a6ff"}
 :strategy   {:shape :square  :color "#3fb950"}
 :regime     {:shape :diamond :color "#d29922"}
 :dataset    {:shape :hexagon :color "#f85149"}}

;; Edge types
{:tested-by   {:style :dashed :meaning "strategy tested on dataset"}
 :performs-in {:style :solid  :meaning "strategy performance in regime"}
 :supports    {:style :arrow  :color "#3fb950" :meaning "evidence supports"}
 :contradicts {:style :arrow  :color "#f85149" :meaning "evidence contradicts"}}

;; Edge properties
;; thickness = confidence (STV C)
;; opacity = recency (fades with age)
```

Computed by `src/epistemic.rs`:
```rust
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub weight: f64,  // confidence
    pub value: f64,   // strength (for supports/contradicts)
}

pub fn compute_relationships(
    hypotheses: &[Hypothesis],
    bench: &BenchReport,
    wf: &WalkForwardResult,
) -> Vec<Relationship>
```

Force simulation: vanilla JS (~100 lines), spring-charge model. No d3 dependency.

### 3. Session Templates (X1)

Structured experiment definitions:

```json
{
  "name": "friction-dominance",
  "hypotheses": ["H002", "H008"],
  "datasets": ["btc_real_1h.csv", "btc_bull_1h.csv"],
  "strategies": "all",
  "success_criteria": {
    "h002_strength_above": 0.8,
    "friction_ratio_above": 2.0
  }
}
```

Three starter sessions:
- **friction-dominance** — test H002 and H008 across regimes
- **capital-preservation** — test H003 and H009 in bear markets
- **regime-sensitivity** — test all hypotheses across all regimes

New binary: `src/bin/session.rs` — reads session JSON, runs backtests, evaluates criteria, produces session report.

### 4. Reconciliation Workflow (X2)

Three-phase integrity check:

```
Phase 1: INTEGRITY
  - Run trap_status() → compute guard gaps
  - Run invariant checks → verify epistemic consistency
  - Compare config hash → detect drift

Phase 2: GAP ANALYSIS
  - Untested hypotheses (uncertainty_map.untested)
  - Unguarded traps (trap_status where guard == Unguarded)
  - Missing datasets (regimes without coverage)
  - Stale evidence (JSONL entries older than N days)

Phase 3: ACTION PLAN
  - Prioritized list of remediation steps
  - Each step has: description, estimated effort, impact on integrity_score
  - Output: out/reconciliation/{date}.json
```

### 5. Dead Code Resolution

**v2.0 will ship without:**
- `strategies.rs` (808 lines) — archived unless benchmarked
- `signals.rs` (459 lines) — archived with strategies.rs
- `filters.rs` (300 lines) — archived with strategies.rs
- `sizing.rs` — archived (Kelly criterion lives in risk.rs)
- Engine loop (`skeleton/`) — archived unless event-sourced path is pursued

**Total removal:** ~2,000 lines of dead code → cleaner module map, faster compilation.

### 6. Config File Support

```toml
# arbitragefx.toml
[strategy]
entry_threshold = 1.0
edge_scale = 0.002
take_profit = 0.01
stop_loss = 0.006
min_hold_candles = 3

[risk]
max_position_pct = 0.10
max_daily_loss_pct = 0.05

[execution]
fee_rate = 0.001
slippage_k = 0.0005
mode = "market"  # instant | market | limit | realistic

[data]
symbol = "BTC/USDT"
```

- Config hash computed from TOML content
- Environment variables still override TOML values
- `--config path/to/config.toml` CLI argument
- Config snapshots saved to `out/configs/{hash}.toml`

---

## Ecosystem Integration

### With honeycomb (port 7777)
ArbitrageFX registers with honeycomb's constellation grid:
```json
{"name": "arbitragefx", "port": 51723, "health": "/api/health",
 "type": "workbench", "description": "Hypothesis-driven backtesting"}
```

### With fancy (liveness registry)
ArbitrageFX emits liveness events:
```json
{"service": "arbitragefx", "event": "pipeline_complete",
 "integrity_score": "9/18", "tests": 281, "hypotheses": 9}
```

### With journal (development chronicle)
Pipeline output feeds journal entries:
```json
{"date": "2026-02-20", "project": "arbitragefx",
 "summary": "Phase 1 complete: 6/6 workbench affordances delivered",
 "evidence": ["281 tests", "9/18 integrity", "3 STV updates"]}
```

### With theirtents (UI zones)
ArbitrageFX occupies the "war-room" zone in theirtents:
- Hypothesis status → war-room left panel
- Active session → war-room center
- Trap checklist → war-room right panel

---

## Release Plan

### v2.0-alpha (Phase 1: Cleanup)
- Archive dead code (strategies.rs, signals.rs, filters.rs, engine/)
- Add config file support (TOML)
- Add `/api/graph`, `/api/traps`, `/api/timeline/{id}` endpoints
- Smoke tests for new endpoints

### v2.0-beta (Phase 2: CLJS + Graph)
- ClojureScript observation layer (shadow-cljs + Reagent)
- Ontology graph with force-directed layout
- Session templates (3 starter sessions)
- Activity feed

### v2.0-rc (Phase 3: Integration)
- Reconciliation workflow
- Ecosystem integration (honeycomb, fancy, journal)
- Reproduce binary for deterministic replay verification
- Evidence provenance (run_id tracing)

### v2.0 (Phase 4: Publication)
- Comprehensive documentation update
- Performance benchmarks across all phases
- Guarantee surface expansion (target: 14/18 traps guarded)
- GitHub Pages deployment of CLJS app

---

## Metrics for Success

| Metric | v1.x (current) | v2.0 target |
|--------|----------------|-------------|
| Tests | 281 | 350+ |
| Integrity score | 9/18 | 14/18 |
| Library LOC | 15,700 | 12,000 (after dead code removal) |
| Hypotheses tested | 9 | 12+ |
| API endpoints | 4 | 10+ |
| Observation surfaces | 1 (workbench) | 4 (workbench, graph, sessions, reconciliation) |
| Dead code | ~2,000 lines | 0 |
| Config reproducibility | SHA256 hash only | Full TOML snapshot |

---

## Non-Goals

- **Live trading in v2.0** — requires calibrated execution model, which requires live data
- **Multiple asset support** — stay focused on BTC/USDT until hypotheses are resolved
- **Performance optimization** — 50k candles/sec is sufficient for research
- **Mobile UI** — desktop-first for research workbench
- **User authentication** — single-user research tool

---

*v2.0 is not about adding features. It is about making the existing epistemic infrastructure observable, composable, and honest — and removing the parts that aren't.*
