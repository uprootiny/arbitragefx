# UI Assessment — feature/ui-mapping

**Context**
- The core binary (`engine_loop`) is an event-driven deterministic engine with regime-aware risk controls, logging via structured JSON, a WAL for replay, and command-based execution.
- Current CLI feedback is limited to `stderr` prints; there is no dedicated UI for monitoring regimes, invariants, alerts, or trace summaries.
- Key system affordances: regime labeling (grounded/uncertain/narrative/reflexive), invariant enforcement (halts on risk/latency/liquidity breaches), structured logs (`events.jsonl`, metrics), and `EngineState` hashes for reproducible diagnostics.

**UI Opportunities**
1. **Status Dashboard (Traceable UI)**
   - Show current regime with confidence/narrative score, drift indicator, staleness age, and active invariants.
   - Display active intents/commands plus reason codes, last risk violations, open positions, exposure, and AWD/latency budgets.
   - Pull from `out/runs/<run_id>/events.jsonl` and the live `EngineState` to create a streaming summary.

2. **Investigation Panel**
   - When `Command::Halt` fires, emit an incident record; UI surfaces the timeline (trigger reversal, health status, required investigation steps).
   - Link to WAL hashes + logs for deterministic replay of the state leading to the halt.

3. **Regime Explorer**
   - Visualize regime history (e.g., timeline of `NarrativeRegime` changes) and associated defensive actions taken (size reductions, halts).
   - Highlight when `regime.is_stale` triggered conservative multipliers.

4. **Log Summaries & Metrics**
   - Hourly/daily summaries (per logging spec) aggregated into UI tiles, showing trades, draws, drift corrections, and reliability events.
   - Allow filtering by domain (strategy, exec, risk) and severity for quick forensic review.

5. **AI Guidance Pane**
   - Present `ai.brief`-style snapshot: health (green/yellow/red), next allowed actions, top risks near limits, last 5 events.
   - Embed toggles for trace logs (on-demand) and manual overrides from planning branch (with documented `allow_override` guard).

**Next steps for UI branch**
- Link logging outputs (`events.jsonl`, `metrics.jsonl`) to a simple web/terminal UI using lightweight Rust HTTP server or TUI.
- Provide a `ui serve` subcommand that reads runtime state summaries and streams them over local websocket/HTTP for remote cockpit.
- Seed placeholder data/stubs from existing docs (roadmap, logging specs) to show potential layout.
- Treat the UI as read-only for now; integrate controls only once invariants/commands stabilized.
