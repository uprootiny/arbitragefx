# Live Readiness Checklist

Ordered by dependency. Each layer assumes the ones above are solid.

## 0. Core Data Integrity
- Market data freshness gate
- Aux data freshness gate + backoff
- Missing data -> unknown (no implicit zeros)

## 1. Order Lifecycle Correctness
- Order state machine
- Idempotent submit
- Reject/Cancel/Timeout handling
- Partial fill handling

## 2. Execution Plumbing
- Place order (live)
- Cancel / cancel-all
- Fills ingestion (WS primary + poll fallback)
- Open orders reconciliation
- Position/balance reconciliation

## 3. Risk Enforcement
- Position caps
- Daily loss halt (realized + unrealized)
- Cooldown enforcement
- Circuit breaker (API failures, stale feeds)

## 4. State Persistence + Recovery
- WAL: place/fill/snapshot
- Replay -> identical state
- Recovery of pending orders

## 5. Strategy Gatekeeping
- Strategy declares required feeds
- Strategy blocked if required feeds missing
- Strategy blocked if regime/drift says no-trade

## 6. Observability
- Structured logs for strategy/risk/exec/fill
- Profile scopes for hot paths
- Health status (WS/poll mode)

## 7. Deployment Safety
- Paper vs live configs separated
- Env-file for secrets
- systemd unit with restart/backoff
- Log rotation

## 8. Final Live Readiness Tests
- Simulated WS drop -> poll-only works
- Reconcile mismatch -> halt trading
- Restart -> WAL replay correct
- Cancel-all works
