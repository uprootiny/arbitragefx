# Live Readiness Dependency Tree

```
Live Trading Readiness
├─ Core Data Integrity
│  ├─ Market freshness gate
│  ├─ Aux freshness gate + backoff
│  └─ Missing data -> unknown (no zero defaults)
├─ Order Lifecycle Correctness
│  ├─ Order state machine
│  ├─ Idempotent submit
│  ├─ Reject/Cancel/Timeout
│  └─ Partial fills
├─ Execution Plumbing
│  ├─ Place order (live)
│  ├─ Cancel / cancel-all
│  ├─ Fills ingestion (WS primary)
│  ├─ Fills ingestion (poll fallback)
│  ├─ Open orders reconcile
│  └─ Position/balance reconcile
├─ Risk Enforcement
│  ├─ Position caps
│  ├─ Daily loss halt
│  ├─ Cooldown
│  └─ Circuit breaker
├─ Persistence + Recovery
│  ├─ WAL place/fill/snapshot
│  ├─ Replay -> identical state
│  └─ Pending orders recovery
├─ Strategy Gatekeeping
│  ├─ Required feeds declared
│  ├─ Missing feeds -> hold
│  └─ Regime/drift -> no-trade
├─ Observability
│  ├─ Structured logs
│  ├─ Profile scopes
│  └─ Health mode (WS/poll)
├─ Deployment Safety
│  ├─ Separate paper/live configs
│  ├─ Env-file secrets
│  ├─ systemd unit
│  └─ Log rotation
└─ Final Live Tests
   ├─ WS drop -> poll-only
   ├─ Reconcile mismatch -> halt
   ├─ Restart -> WAL replay
   └─ Cancel-all works
```
