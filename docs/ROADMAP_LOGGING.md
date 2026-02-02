# Logging-Hardening Roadmap (≈ 48 tickets)

**Focus:** make logging canon (events+trace+metrics), ensure redaction, summaries, AI brief, and replay.

## Sections

1. **Schema & Run Management** (15 tickets)
   - L-001 [S] finalize event schema w/ required fields  
   - L-002 [I] implement run manifest writer  
   - L-003 [T] manifest contains git hash + config hash  
   - L-004 [T] run_id deterministic  
   - L-005 [I] run dir creation + rotation  
   - L-006 [S] log volume budget  
   - L-007 [T] adherence to schema  
   - L-008 [I] redaction middleware  
   - L-009 [T] redaction strips API keys  
   - L-010 [T] redaction strips signatures  
   - L-011 [I] correlation ID pipeline  
   - L-012 [T] events include corr_id  
   - L-013 [I] event taxonomy enum  
   - L-014 [T] enum usage enforced  
   - L-015 [S] redaction policy test plan

2. **Event & Trace Streams** (15 tickets)
   - L-016 [I] canonical event emitter  
   - L-017 [T] events written to events.jsonl  
   - L-018 [I] trace emitter toggle  
   - L-019 [T] trace only when enabled  
   - L-020 [T] trace sampling consistent  
   - L-021 [I] metrics log writer  
   - L-022 [T] metrics log includes exposure  
   - L-023 [T] metrics log includes slippage  
   - L-024 [I] log flush hook  
   - L-025 [T] log flush on shutdown  
   - L-026 [T] log flush on fatal error  
   - L-027 [I] event writer lock proof  
   - L-028 [T] writes under contention  
   - L-029 [I] event replay CLI  
   - L-030 [T] replay reconstructs seq order

3. **Summaries & AI Brief** (18 tickets)
   - L-031 [I] hourly rollup generator  
   - L-032 [T] hourly rollup counts trades  
   - L-033 [T] hourly rollup exposes anomalies  
   - L-034 [I] daily summary emitter  
   - L-035 [T] daily summary includes drawdown  
   - L-036 [T] summary includes regime stats  
   - L-037 [I] AI brief emitter  
   - L-038 [T] brief lists unknowns  
   - L-039 [T] brief lists top risks  
   - L-040 [I] brief includes allowed actions  
   - L-041 [T] brief emitted each minute  
   - L-042 [I] log volume guard  
   - L-043 [T] guard drops low severity  
   - L-044 [T] guard honors budget  
   - L-045 [S] summary schema spec  
   - L-046 [S] AI operator checklist  
   - L-047 [T] AI brief includes last 10 event IDs  
   - L-048 [S] evaluation plan for AI briefs
