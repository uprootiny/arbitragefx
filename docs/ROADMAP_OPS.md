# Ops-Hardening Roadmap (≈ 46 tickets)

**Focus:** risk, reconciliation, deployment, kill switch, ops controls.

## Sections

1. **Risk & Guardrails** (16)
   - O-001 [S] risk rulebook  
   - O-002 [I] risk rule registry  
   - O-003 [T] exposures capped  
   - O-004 [T] daily loss enforced  
   - O-005 [T] trades/day enforced  
   - O-006 [T] cooldown enforced  
   - O-007 [I] slippage guard  
   - O-008 [T] slippage guard halts  
   - O-009 [I] drift guard  
   - O-010 [T] drift guard halts  
   - O-011 [I] staleness guard  
   - O-012 [T] staleness guard blocks  
   - O-013 [I] safe-mode state machine  
   - O-014 [T] safe-mode prohibits new positions  
   - O-015 [T] safe-mode allows close  
   - O-016 [S] safe-mode activation policy

2. **Reconciliation & Drift** (14)
   - O-017 [S] reconcile cadence spec  
   - O-018 [I] reconcile worker  
   - O-019 [T] reconcile on cadence  
   - O-020 [T] reconcile handles missing data  
   - O-021 [T] reconcile logs drift  
   - O-022 [I] drift score calculator  
   - O-023 [T] drift severity computed  
   - O-024 [I] auto halt on drift  
   - O-025 [T] drift triggers halt  
   - O-026 [I] reconcile summary emitter  
   - O-027 [T] summary includes drift deltas  
   - O-028 [T] reconcile matches open orders  
   - O-029 [T] reconcile matches balances  
   - O-030 [S] drift correction policy

3. **Deployment & Observability** (16)
   - O-031 [S] deploy profile spec  
   - O-032 [I] systemd unit templates  
   - O-033 [T] systemd unit loads  
   - O-034 [I] deploy scripts  
   - O-035 [T] upgrade path safe  
   - O-036 [T] rollback path safe  
   - O-037 [I] kill switch handler  
   - O-038 [T] kill switch stops trading  
   - O-039 [I] opt-in live guard  
   - O-040 [T] live config requires opt-in  
   - O-041 [I] health endpoint  
   - O-042 [T] health OK/degraded  
   - O-043 [I] monitoring hook  
   - O-044 [T] monitoring ping  
   - O-045 [S] incident response runbook  
   - O-046 [T] runbook scenario test
