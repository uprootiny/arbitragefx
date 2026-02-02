# Logging Architecture

**Date**: 2026-02-02

## Design Philosophy

Following Unix philosophy, the logging system produces **text streams** as the universal interface. All UI, analysis, and AI guidance derive from parsing these logs.

## Multi-Scale Verification

The system enables coherence verification at three scales:

| Scale | Granularity | Purpose | Tool |
|-------|------------|---------|------|
| MICRO | Signal→Fill pairs | Individual trade integrity | `verify_coherence` |
| MESO | Session invariants | Checkpoint chains, PnL trajectory | `verify_coherence` |
| MACRO | Cross-run hashes | Replay reproducibility | `verify_coherence` |

## Log Levels

```
TRACE  → Every candle, indicator update (trace.jsonl)
DEBUG  → Every signal, risk check, market context (trace.jsonl)
INFO   → Decisions, fills, checkpoints (events.jsonl)
WARN   → Drift detection, guard triggers (events.jsonl)
ERROR  → Failed orders, system issues (events.jsonl)
FATAL  → Unrecoverable failures (events.jsonl)
```

## Log Domains

```
market    → Price data, candles, indicators
strategy  → Signal generation, decisions
risk      → Guard checks, position limits
exec      → Order lifecycle
fill      → Fill processing, PnL reconciliation
drift     → Distribution drift detection
system    → Startup, shutdown, recovery
audit     → Replay/audit trail
agent     → AI guidance fields
```

## Output Structure

Each run creates a directory under `out/runs/<run_id>/`:

```
out/runs/r-1234567890-12345/
├── manifest.json       # Run metadata
├── events.jsonl        # INFO+ events (decisions, fills, checkpoints)
├── trace.jsonl         # DEBUG events (signals, guards, context)
└── metrics.jsonl       # Periodic metric summaries
```

## Log Entry Format

```json
{
  "ts": "2026-02-02T12:10:39.635Z",
  "run_id": "r-1234567890-12345",
  "seq": 42,
  "lvl": "INFO",
  "component": "fill",
  "event": "fill",
  "msg": "",
  "strategy_id": "mom-0",
  "data": {
    "price": 105000.00,
    "qty": 0.001,
    "fee": 0.105,
    "realized_pnl": 2.34
  }
}
```

## AI Agent Guidance

Special log events for AI agent understanding:

### `decision` (Domain: agent)
```json
{
  "intent": "buy",
  "reason": "trending. none",
  "confidence": 0.75,
  "alternatives": [{"action": "hold", "score": 0.0}, {"action": "buy", "score": 1.5}],
  "state_hash": "abc123..."
}
```

### `market_context` (Domain: agent)
```json
{
  "symbol": "BTCUSDT",
  "price": 105000.00,
  "z_momentum": 1.5,
  "z_vol": 0.8,
  "funding_rate": 0.0001,
  "regime": "trending",
  "drift_severity": "none"
}
```

### `reasoning` (Domain: agent)
```json
{
  "strategy_id": "mom-0",
  "steps": [
    "Signal: buy (score=1.50)",
    "Risk check: passed",
    "Fill: qty=0.001 @ 105000.00"
  ]
}
```

## Tools

### `logparse` - Log analysis CLI

```bash
# Summary statistics
logparse summary events.jsonl

# Filter by domain and level
logparse filter events.jsonl --domain=agent --level=info

# Check replay determinism
logparse replay events.jsonl

# Audit trail integrity
logparse audit events.jsonl

# Extract time slice
logparse slice events.jsonl "2026-02-02T12:00:00Z" "2026-02-02T13:00:00Z"

# AI agent summary
logparse agent events.jsonl
```

### `verify_coherence` - Multi-scale verification

```bash
verify_coherence out/runs/r-1234567890-12345/
```

Verifies:
- MICRO: Decision→fill pairs match
- MESO: Sequence continuity, checkpoint chains
- MACRO: State hash uniqueness

## Environment Variables

```bash
LOG_LEVEL=debug         # Minimum log level (trace/debug/info/warn/error/fatal)
LOG_DOMAINS=all         # Comma-separated list or "all"
LOG_DIR=out/runs        # Base directory for log output
RUN_ID=r-custom         # Override auto-generated run ID
LOG_FLUSH_SECS=300      # Periodic aggregation interval
PROFILE_SAMPLE=0.1      # Profile sampling rate (0.0-1.0)
```

## Example: Analyzing a Backtest

```bash
# Run backtest with detailed logging
LOG_LEVEL=debug ./target/release/backtest_logged data/btc_1h_180d.csv

# Find the run directory
RUN=$(ls -1t out/runs/ | head -1)

# Summary statistics
./target/release/logparse summary out/runs/$RUN/events.jsonl

# AI agent analysis
./target/release/logparse agent out/runs/$RUN/events.jsonl

# Verify coherence at all scales
./target/release/verify_coherence out/runs/$RUN/

# Filter to just drift events
./target/release/logparse filter out/runs/$RUN/events.jsonl --domain=drift --level=warn

# Replay validation
./target/release/logparse replay out/runs/$RUN/events.jsonl
```

## Replay Support

The logging system supports deterministic replay via:

1. **Sequence numbers**: Monotonic counter for ordering
2. **State hashes**: Checkpoint hashes for verification
3. **Input hashes**: Hash of market data at decision points
4. **Output hashes**: Hash of resulting state

To verify replay determinism:
```bash
# Run twice with same data
./target/release/backtest_logged data/btc.csv
./target/release/backtest_logged data/btc.csv

# Compare final state hashes
./target/release/logparse audit out/runs/r-1/events.jsonl
./target/release/logparse audit out/runs/r-2/events.jsonl
```

## Scaffolding Philosophy

The logging layers are designed to be **detachable** - they enable verification at varied scales without coupling the core trading logic to any specific analysis tool. This follows the Unix principle of:

> Write programs to handle text streams, because that is a universal interface.

All verification, summarization, and AI guidance derive from parsing the same log streams, allowing independent evolution of analysis tools.
