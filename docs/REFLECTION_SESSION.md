# Session Reflection: Lessons for CLAUDE.md

**Date:** 2026-02-20
**Context:** Extended session building Phase 1 workbench affordances — from smoke tests through CI-validated deployment with live URLs.

---

## What Happened

This session traversed the full arc from plan to production: 28 smoke tests, a bench profiler, a self-contained workbench dashboard generator, pipeline integration, CI green, GitHub Pages live, epistemic server enriched with health/state/workbench endpoints. Six commits, each validated against CI before the next.

The deeper pattern: we moved from "the system works" to "the system is observable." The workbench became the surface where hypothesis truth values, trap integrity, regime performance, and resource trends converge into a single artifact. Not a dashboard — an observation surface.

---

## Lessons Worth Encoding

### 1. CI-First Commit Discipline

Every commit was built, tested, formatted, and pushed before moving to the next concern. When `cargo fmt --check` failed in CI, we formatted the entire codebase (63 files) and committed that as a separate atomic commit. This prevented format debt from accumulating.

**Encode:** Commit atomically. Each commit should be a single coherent change that passes CI independently. Format violations are not technical debt — they are broken builds.

### 2. The Observation Surface Pattern

The workbench is not a UI — it is a lens. Every data source (bench JSON, walk-forward report, hypothesis ledger, JSONL history, trap status, uncertainty map) feeds into a single self-contained HTML artifact. No build tools. No external dependencies. Just an embedded JSON blob and vanilla JS.

**Encode:** Prefer self-contained artifacts over build pipelines. A 174KB HTML file with embedded data is more valuable than a React app requiring node_modules. The artifact should be deployable by `cp`.

### 3. Keystone Changes Enable Cascades

The JSONL history append (`out/ledger_history/updates.jsonl`) was identified as the "keystone change" in the plan. It was small (~30 lines added to update_ledger.rs) but it enabled: temporal sparklines, evidence provenance, audit trails, and future reconciliation workflows. A single well-placed data structure unlocked multiple downstream affordances.

**Encode:** Identify keystone changes — the smallest edit that enables the most downstream value. Implement those first. The right 30 lines are worth more than 300 lines of UI.

### 4. Enrichment Over Replacement

The `/api/health` endpoint went from `{"status":"ok"}` to a 7-field response with test count, integrity score, dataset inventory, pipeline freshness, invariant status. The epistemic server gained a root route serving the full workbench dashboard. None of this replaced existing functionality — it enriched surfaces that already existed.

**Encode:** Prefer enriching existing endpoints over creating new ones. An existing route that returns richer data is more discoverable than a new route that returns the same data.

### 5. Guard Status as Computed Property

The `trap_status()` function returns all 18 backtest traps with their guard status (Guarded/Partial/Unguarded). The `integrity_score()` function computes a simple ratio. These are not stored — they are computed from the current state of the codebase. When we add a new guard (say, implementing walk-forward validation), the score improves automatically.

**Encode:** Integrity metrics should be computed, not declared. A function that scans the codebase for evidence of guards is more honest than a manually maintained checklist.

### 6. The Port Discovery Problem

We tried ports 8765 (in use), 8766 (timeout), 8767, 8768, 9847 (Java), before finding 51723 free. Each failed attempt cost time and context. The user's suggestion of "high uncontested port" is the right heuristic.

**Encode:** Default to high random ports (50000+) for development servers. Check `ss -tlnp` before binding. Make port configurable via environment variable with a high default.

### 7. Parallel Agent Architecture

The session used parallel Explore agents effectively: 3 agents simultaneously surveyed user-facing workflows, guarantee surface, and strategy/hypothesis space. This produced a comprehensive plan in one round-trip rather than three sequential explorations.

**Encode:** When exploring a codebase for planning, launch multiple focused agents in parallel rather than one comprehensive agent. Specificity beats breadth for agent tasks.

### 8. The GitHub Pages Dance

Pages deployment required: public repo, environment creation, branch policy, and a non-broken workflow file. The original workflow had a YAML heredoc that may have caused parsing issues. The fix was simplification: copy pre-generated files rather than generate in the workflow.

**Encode:** CI/CD workflows should be minimal — just copy artifacts that were generated locally. Don't generate HTML in YAML heredocs. Pre-generate, commit, copy.

---

## Patterns to Avoid

### Over-Abstraction in Templates
The workbench HTML template is a single large string constant in Rust (`TEMPLATE`). This is ugly but correct — it means the generator is a single binary with zero runtime dependencies. The temptation to extract it into a template file would add a deployment dependency.

### Interactive Endpoints in Non-Interactive Servers
The epistemic server is synchronous and single-threaded. Running `cargo test --list` on every `/api/health` request blocked the server for seconds. The fix was to pre-compute expensive values at startup and serve cached results.

### Path Filters That Are Too Narrow
Adding `paths: ["docs/**"]` to the Pages workflow meant it only triggered on docs changes. This prevented Pages from deploying when we only changed the workflow file. The fix was including the workflow file itself in the path filter.

---

## What Should Go Into CLAUDE.md

### For This Project Specifically

```
## ArbitrageFX Project Conventions

- All commits must pass: cargo fmt, cargo clippy, cargo test, smoke tests
- Workbench dashboard is the canonical observation surface
- Hypothesis truth values (stv strength confidence) are the primary evidence format
- Integrity score (guarded/total traps) tracks guarantee surface expansion
- JSONL append-only history for temporal tracking
- Self-contained HTML artifacts (embedded JSON, vanilla JS, no build tools)
- Default server port: 51723 (configurable via PORT env var)
- Zero new dependencies — all work uses existing serde_json, sha2, chrono, std
```

### For General Development Practice

```
## Development Preferences

- Commit atomically with CI validation between each commit
- Prefer enriching existing surfaces over creating new ones
- Identify and implement keystone changes first
- Compute integrity metrics, don't declare them
- Use high random ports (50000+) for development servers
- CI workflows should copy pre-generated artifacts, not generate them
- Self-contained artifacts over build pipelines
- Observation surfaces over dashboards
```

---

## The Deeper Pattern

This session's trajectory follows the CLAUDE.md gradient vectors:

- **Ontology Over Accident** (1): Named types for StvHistoryEntry, TrapStatusEntry, UncertaintyMap, GuardStatus — not ad-hoc JSON
- **Observability Over Mystery** (10): Every internal state now has a surface — hypotheses, traps, regimes, trends
- **Information Preservation Over Loss** (14): JSONL append-only log preserves temporal history
- **Reproducibility Over Environment Luck** (8): Config hash, deterministic replay, SHA256 data provenance
- **Proof Over Assumption** (4): `trap_status()` computes guard state from evidence, not declaration

The net drift is toward structure. Each commit expanded the guarantee surface while maintaining all existing tests. The system now has 281 tests, 9/18 traps guarded, temporal evidence tracking, and a live observation surface at both `localhost:51723` and `https://uprootiny.github.io/arbitragefx/`.

---

*The workbench is not the map. The workbench is the instrument by which the territory becomes legible.*
