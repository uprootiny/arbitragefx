# 672-Ticket Virtue-Aligned Roadmap (60/20/20)

Counts:
- Tests: 403
- Specs/PM/Integration: 134
- Implementation: 135

Outlier weight:
- Backtesting + execution realism
- Logging/forensics + replay

Legend:
- [T] Test
- [S] Spec/PM/Integration
- [I] Implementation

---

## 1) CORE ENGINE (84)

### Specs / PM / Integration (16)
S-001 [S] Define core invariants and enforcement points  
S-002 [S] Formalize event ordering contract  
S-003 [S] Single-mutation-site design spec  
S-004 [S] WAL-before-state policy spec  
S-005 [S] Replay correctness criteria  
S-006 [S] Risk gating contract  
S-007 [S] Order lifecycle state map  
S-008 [S] Intent ID standardization  
S-009 [S] State hash definition  
S-010 [S] Engine halt states taxonomy  
S-011 [S] Time/cadence contract  
S-012 [S] Failure mode classification  
S-013 [S] Recovery state reconciliation spec  
S-014 [S] Safe mode behavior spec  
S-015 [S] Mandatory “unknown” state semantics  
S-016 [S] Module boundary policy

### Tests (50)
T-001 [T] State mutation only via engine loop  
T-002 [T] WAL entry precedes state mutation  
T-003 [T] Replay reproduces state hash  
T-004 [T] Event seq monotonicity  
T-005 [T] Idempotent order intent  
T-006 [T] Duplicate fill dedupe  
T-007 [T] Halt prevents new orders  
T-008 [T] Close allowed during halt  
T-009 [T] Risk can only block  
T-010 [T] Recovery from empty WAL  
T-011 [T] Recovery from partial WAL  
T-012 [T] Engine handles missing candle data  
T-013 [T] Engine handles missing aux data  
T-014 [T] Engine handles stale aux data  
T-015 [T] Engine handles time rollback  
T-016 [T] Engine handles clock drift  
T-017 [T] Cooldown enforcement  
T-018 [T] Daily counters reset on day change  
T-019 [T] Exposure clamp correctness  
T-020 [T] Equity recompute correctness  
T-021 [T] Entry price weighted average  
T-022 [T] Flip position entry reset  
T-023 [T] Position zero resets entry  
T-024 [T] State hash stable across runs  
T-025 [T] Forced close on last bar  
T-026 [T] Strategy update called once per bar  
T-027 [T] No trade if market data missing  
T-028 [T] No trade if critical aux missing  
T-029 [T] Event bus drain correctness  
T-030 [T] Retry respects max attempts  
T-031 [T] Retry backoff monotonic  
T-032 [T] Cancel request results in cancel ack  
T-033 [T] Cancel all cancels each order  
T-034 [T] Order state: submit→ack→fill  
T-035 [T] Order state: submit→reject  
T-036 [T] Order state: submit→cancel  
T-037 [T] Order state: ack→partial→fill  
T-038 [T] Unmatched fill handling  
T-039 [T] Intent ID unique per strategy  
T-040 [T] Client order ID unique per strategy  
T-041 [T] WAL snapshot per strategy  
T-042 [T] WAL replay per strategy  
T-043 [T] Circuit breaker activates on error rate  
T-044 [T] Circuit breaker clears on recovery  
T-045 [T] Drift halt triggers safe mode  
T-046 [T] Drift close triggers forced exit  
T-047 [T] Safe mode prevents new intents  
T-048 [T] Safe mode only allows close  
T-049 [T] State transition audit log emitted  
T-050 [T] Engine loop runs with empty strategy set

### Implementation (18)
I-001 [I] Extract core loop into engine_core  
I-002 [I] Add state hash helper  
I-003 [I] Add event seq to all events  
I-004 [I] Add deterministic run_id  
I-005 [I] Implement safe mode transitions  
I-006 [I] Add close-only mode  
I-007 [I] Add unknown regime state  
I-008 [I] Add optional run manifest  
I-009 [I] Add kill file support  
I-010 [I] Add “trade denied” event  
I-011 [I] Add event dedupe key store  
I-012 [I] Add per-strategy risk state  
I-013 [I] Add state checkpoint markers  
I-014 [I] Add shutdown hook to flush WAL  
I-015 [I] Add structured engine errors  
I-016 [I] Consolidate duplicate types  
I-017 [I] Reduce main.rs to <400 LOC  
I-018 [I] Make core loop library-callable

---

## 2) LOGGING + FORENSICS (92)

### Specs / PM / Integration (20)
S-017 [S] Canonical log schema (event/trace/metrics)  
S-018 [S] Event taxonomy + stable IDs  
S-019 [S] Redaction policy spec  
S-020 [S] Run directory layout spec  
S-021 [S] Log retention policy  
S-022 [S] Audit trail requirements  
S-023 [S] Replay narrative requirements  
S-024 [S] Summary rollup schema  
S-025 [S] AI operator brief schema  
S-026 [S] Required INFO-level coverage  
S-027 [S] Trace enablement policy  
S-028 [S] Error classification taxonomy  
S-029 [S] Incident severity levels  
S-030 [S] Correlation ID propagation spec  
S-031 [S] Metrics snapshot cadence spec  
S-032 [S] Log volume budget  
S-033 [S] Field whitelist policy  
S-034 [S] “No silent fallback” logging rule  
S-035 [S] Postmortem report template  
S-036 [S] Operator dashboard minimums

### Tests (54)
T-051 [T] Log includes run_id and seq  
T-052 [T] Log includes event taxonomy  
T-053 [T] Redaction removes API keys  
T-054 [T] Redaction removes signatures  
T-055 [T] Events written to events.jsonl  
T-056 [T] Trace logs to trace.jsonl  
T-057 [T] Metrics logs to metrics.jsonl  
T-058 [T] Log files created on first event  
T-059 [T] Manifest written once per run  
T-060 [T] Rollup emitted hourly  
T-061 [T] Rollup includes counts  
T-062 [T] Rollup includes worst incidents  
T-063 [T] AI brief includes unknowns  
T-064 [T] AI brief includes top risks  
T-065 [T] Risk gate always logged  
T-066 [T] Order lifecycle always logged  
T-067 [T] Reconcile drift logged  
T-068 [T] Circuit breaker logged  
T-069 [T] Stale feed logged  
T-070 [T] Missing feed logged  
T-071 [T] Recovery logged  
T-072 [T] WAL snapshot logged  
T-073 [T] Summary contains regime stats  
T-074 [T] Summary contains drawdown duration  
T-075 [T] Error logs include corr_id  
T-076 [T] Event log size limit respected  
T-077 [T] Trace log disabled by default  
T-078 [T] Trace log sampling works  
T-079 [T] Fields omitted are not logged  
T-080 [T] Event log replay yields same count  
T-081 [T] Event log sorts by seq  
T-082 [T] Order state transitions logged  
T-083 [T] Log injection safe (no newline)  
T-084 [T] JSON lines parse correctly  
T-085 [T] Invalid log record rejected  
T-086 [T] Log flush on shutdown  
T-087 [T] Log flush on fatal error  
T-088 [T] Rollup does not leak secrets  
T-089 [T] Metrics snapshot cadence  
T-090 [T] Log records include strategy_id when available  
T-091 [T] Strategy signals contain reason codes  
T-092 [T] Log severity classification  
T-093 [T] Log event coverage per component  
T-094 [T] Replay narrative reconstructs sequence  
T-095 [T] Metrics log includes slippage est  
T-096 [T] Metrics log includes exposure

### Implementation (18)
I-019 [I] Canonical log emitter helper  
I-020 [I] Add run manifest writer  
I-021 [I] Add event taxonomy enum  
I-022 [I] Add correlation ID pipeline  
I-023 [I] Add log rollup scheduler  
I-024 [I] Add AI brief emitter  
I-025 [I] Add log volume guard  
I-026 [I] Add log schema validation  
I-027 [I] Add log compression option  
I-028 [I] Add log file rotation  
I-029 [I] Add event log replay tool  
I-030 [I] Add summary generator  
I-031 [I] Add incident recorder  
I-032 [I] Add metrics snapshotter  
I-033 [I] Add log anomaly detector  
I-034 [I] Add audit log writer  
I-035 [I] Add redaction middleware  
I-036 [I] Add “INFO must be sufficient” checks

---

## 3) WAL + RECOVERY (62)

### Specs / PM / Integration (12)
S-037 [S] WAL schema definition  
S-038 [S] Snapshot cadence policy  
S-039 [S] Recovery guarantees  
S-040 [S] WAL corruption policy  
S-041 [S] Replay boundaries  
S-042 [S] Strategy snapshot format  
S-043 [S] WAL integrity checks  
S-044 [S] WAL compaction policy  
S-045 [S] Crash recovery flow  
S-046 [S] Versioned WAL format  
S-047 [S] WAL security policy  
S-048 [S] WAL audit policy

### Tests (38)
T-097 [T] WAL append integrity  
T-098 [T] WAL replay yields same state  
T-099 [T] WAL with partial line recovery  
T-100 [T] WAL with corrupt line recovery  
T-101 [T] WAL snapshot restores position  
T-102 [T] WAL snapshot restores cash  
T-103 [T] WAL snapshot per strategy  
T-104 [T] WAL replay respects intent order  
T-105 [T] WAL replay with duplicate fills  
T-106 [T] WAL replay with missing fills  
T-107 [T] WAL replay with cancel events  
T-108 [T] WAL replay with reject events  
T-109 [T] WAL replay with partial fills  
T-110 [T] WAL replay with halt events  
T-111 [T] WAL replay idempotent  
T-112 [T] WAL file rotation safe  
T-113 [T] WAL compaction retains state  
T-114 [T] WAL checksum validation  
T-115 [T] WAL snapshot hash matches  
T-116 [T] WAL encoding version mismatch handled  
T-117 [T] WAL handle file permissions  
T-118 [T] WAL flush on shutdown  
T-119 [T] WAL flush on fatal error  
T-120 [T] WAL writer lock safe  
T-121 [T] WAL handles large intents  
T-122 [T] WAL handles long runs  
T-123 [T] WAL replay timing independence  
T-124 [T] WAL replay with clock skew  
T-125 [T] WAL replay with stale aux  
T-126 [T] WAL snapshot rolling  
T-127 [T] WAL recovery from empty file  
T-128 [T] WAL recovery from huge file  
T-129 [T] WAL recovery mismatch error  

### Implementation (12)
I-037 [I] Add WAL version header  
I-038 [I] Add WAL checksum  
I-039 [I] Add WAL rotation  
I-040 [I] Add WAL compaction  
I-041 [I] Add WAL replay tool  
I-042 [I] Add snapshot writer  
I-043 [I] Add snapshot reader  
I-044 [I] Add recovery report  
I-045 [I] Add WAL corruption handler  
I-046 [I] Add WAL scrubber  
I-047 [I] Add WAL integrity CLI  
I-048 [I] Add WAL audit hook  

---

## 4) EXECUTION + EXCHANGE ADAPTERS (92)

### Specs / PM / Integration (20)
S-049 [S] Adapter interface spec  
S-050 [S] Order lifecycle contract  
S-051 [S] Precision/lot rules spec  
S-052 [S] Fee schedule spec  
S-053 [S] Retryable error taxonomy  
S-054 [S] Rate limit policy  
S-055 [S] Cancel/replace policy  
S-056 [S] Slippage guard policy  
S-057 [S] Fill validation policy  
S-058 [S] Cross-venue abstraction boundary  
S-059 [S] Venue reconciliation policy  
S-060 [S] Adapter error normalization  
S-061 [S] Execution reliability requirements  
S-062 [S] Maker/taker logic spec  
S-063 [S] Queue model spec  
S-064 [S] API timeout policy  
S-065 [S] Order dedupe policy  
S-066 [S] Hedge/basis spec  
S-067 [S] Execution journaling requirements  
S-068 [S] Live/paper parity requirements

### Tests (54)
T-130 [T] Place order returns ack  
T-131 [T] Cancel order returns ack  
T-132 [T] Cancel all returns ack  
T-133 [T] Retry on 429  
T-134 [T] Retry on 5xx  
T-135 [T] No retry on 4xx  
T-136 [T] Precision rounding correct  
T-137 [T] Min notional enforced  
T-138 [T] Tick size enforced  
T-139 [T] Maker/taker classification  
T-140 [T] Slippage guard halts  
T-141 [T] Fill validation vs last price  
T-142 [T] Duplicate order intent blocked  
T-143 [T] Idempotency key reuse safe  
T-144 [T] Cancel stale orders  
T-145 [T] Cancel unknown order  
T-146 [T] Partial fills accumulate  
T-147 [T] Order rejected path  
T-148 [T] Order timeout path  
T-149 [T] Exchange disconnect path  
T-150 [T] WebSocket fill parsing  
T-151 [T] Poll fallback fills  
T-152 [T] Execution error logged  
T-153 [T] Execution latency logged  
T-154 [T] Retry backoff logged  
T-155 [T] Order state transitions logged  
T-156 [T] Exec log fields redacted  
T-157 [T] Place order with safe size  
T-158 [T] Reject on stale market data  
T-159 [T] Order lifecycle monotonic  
T-160 [T] Dedupe fills by ID  
T-161 [T] Cancel after fill safe  
T-162 [T] Cancel after reject safe  
T-163 [T] Cancel all while halted  
T-164 [T] Execution parity paper/live  
T-165 [T] Reconcile uses same IDs  
T-166 [T] Execution adapter health check  
T-167 [T] Emergency close path  
T-168 [T] Incorrect fill slip detection  
T-169 [T] Commission calc correctness  
T-170 [T] Fee currency conversions  
T-171 [T] Rate limit backoff  
T-172 [T] Retry exhaustion logs  
T-173 [T] Exchange time sync guard  
T-174 [T] HMAC signing errors handled  
T-175 [T] HTTP error normalization  
T-176 [T] Cancel only reduces open orders  
T-177 [T] Cancel all clears local pending  
T-178 [T] Order exec with max position  
T-179 [T] Exec result stable in error  
T-180 [T] Fill ID uniqueness  
T-181 [T] Order ID mapping consistency  
T-182 [T] Ack before fill ordering  
T-183 [T] Late fill handling  
T-184 [T] Retry excludes non-idempotent  
T-185 [T] API credentials not logged

### Implementation (18)
I-049 [I] Adapter interface consolidation  
I-050 [I] Add venue error normalization  
I-051 [I] Add precision enforcement  
I-052 [I] Add min notional enforcement  
I-053 [I] Add rate limit queue  
I-054 [I] Add WebSocket listener  
I-055 [I] Add polling fallback  
I-056 [I] Add fill dedupe map  
I-057 [I] Add cancel-stale policy  
I-058 [I] Add slippage guard  
I-059 [I] Add execution latency tracker  
I-060 [I] Add retry policy interface  
I-061 [I] Add adapter health probe  
I-062 [I] Add emergency close  
I-063 [I] Add execution journal integration  
I-064 [I] Add maker/taker classification  
I-065 [I] Add per-venue fee rules  
I-066 [I] Add live/paper parity shim

---

## 5) BACKTESTING & REALISM (110)

### Specs / PM / Integration (26)
S-069 [S] Backtest scope definition  
S-070 [S] No lookahead formal rules  
S-071 [S] Latency model spec  
S-072 [S] Slippage model spec  
S-073 [S] Partial fills spec  
S-074 [S] Fee model spec  
S-075 [S] Funding/borrow model spec  
S-076 [S] Regime segmentation spec  
S-077 [S] Walk-forward spec  
S-078 [S] Parameter sweep spec  
S-079 [S] Multiple testing correction spec  
S-080 [S] Backtest failure criteria  
S-081 [S] Baseline strategy spec  
S-082 [S] Results reporting spec  
S-083 [S] “Reality score” definition  
S-084 [S] Backtest manifest schema  
S-085 [S] Dataset acceptance policy  
S-086 [S] Data gap policy  
S-087 [S] Data outlier policy  
S-088 [S] Stress test policy  
S-089 [S] Backtest reproducibility spec  
S-090 [S] Backtest summary contract  
S-091 [S] Stratified report spec  
S-092 [S] Baseline data retention policy  
S-093 [S] Backtest safety guard policy  
S-094 [S] Time-of-day test spec

### Tests (66)
T-186 [T] No lookahead enforcement  
T-187 [T] Latency applied to orders  
T-188 [T] Slippage scales with vol  
T-189 [T] Partial fill model stable  
T-190 [T] Fee model applied correctly  
T-191 [T] Funding applied correctly  
T-192 [T] Borrow applied correctly  
T-193 [T] Walk-forward splits correctly  
T-194 [T] Sweep correction applied  
T-195 [T] Backtest fails on missing data  
T-196 [T] Backtest fails on outlier  
T-197 [T] Backtest fails on mis-schema  
T-198 [T] Backtest summary includes manifest  
T-199 [T] Backtest summary includes PnL  
T-200 [T] Backtest summary includes DD  
T-201 [T] Baseline buy-hold computed  
T-202 [T] Baseline no-trade computed  
T-203 [T] Baseline momentum computed  
T-204 [T] Baseline mean-rev computed  
T-205 [T] Baseline carry computed  
T-206 [T] Regime segmentation applied  
T-207 [T] Regime-conditional metrics output  
T-208 [T] Strategy forced close at end  
T-209 [T] Max drawdown correct  
T-210 [T] Drawdown duration correct  
T-211 [T] Strategy PnL attribution  
T-212 [T] Slippage guard halts backtest  
T-213 [T] High latency suppresses edge  
T-214 [T] Parameter sweep deterministic  
T-215 [T] Multiple trials untrustworthy warning  
T-216 [T] Partial fills reduce PnL  
T-217 [T] Fees reduce PnL  
T-218 [T] Funding improves carry strategy  
T-219 [T] Funding hurts momentum in wrong sign  
T-220 [T] Model handles missing aux  
T-221 [T] Model handles stale aux  
T-222 [T] Low volume increases slippage  
T-223 [T] Event ordering preserved  
T-224 [T] Replayable backtest state hash  
T-225 [T] “Reality score” calculation stable  
T-226 [T] CSV parser rejects bad rows  
T-227 [T] CSV parser accepts valid rows  
T-228 [T] Timezone handling correct  
T-229 [T] Candle cadence consistency  
T-230 [T] Gap detection logic  
T-231 [T] Data dedupe logic  
T-232 [T] Data row count matches expected  
T-233 [T] Baseline report output format  
T-234 [T] Walk-forward window boundaries  
T-235 [T] Parameter sweep output completeness  
T-236 [T] Backtest resource bounds  
T-237 [T] Backtest handles zero volume  
T-238 [T] Backtest handles zero price  
T-239 [T] Volatility regime detection  
T-240 [T] Time-of-day stratification  
T-241 [T] Stress test large slippage  
T-242 [T] Stress test high latency  
T-243 [T] Stress test missing data  
T-244 [T] Stress test delayed fills  
T-245 [T] Stress test large fees  
T-246 [T] Backtest result reproducibility  
T-247 [T] Backtest manifest contains config hash  
T-248 [T] Backtest manifest contains dataset hash  
T-249 [T] Backtest manifest contains git sha  
T-250 [T] Walk-forward report stable  
T-251 [T] Multi-strategy backtest isolation  
T-252 [T] Exposure cap enforced in backtest

### Implementation (18)
I-067 [I] Add latency model  
I-068 [I] Add slippage model  
I-069 [I] Add partial fill model  
I-070 [I] Add funding/borrow model  
I-071 [I] Add regime classifier  
I-072 [I] Add walk-forward harness  
I-073 [I] Add parameter sweep harness  
I-074 [I] Add multiple testing correction  
I-075 [I] Add baseline strategies  
I-076 [I] Add dataset manifest  
I-077 [I] Add backtest summary output  
I-078 [I] Add PnL attribution  
I-079 [I] Add stress test runner  
I-080 [I] Add time-of-day stratifier  
I-081 [I] Add results exporter  
I-082 [I] Add feature provenance hash  
I-083 [I] Add backtest replay tool  
I-084 [I] Add “reality score” metric

---

## 6) RISK + CIRCUIT BREAKERS (60)

### Specs (12)
S-095 [S] Risk guard rulebook  
S-096 [S] Exposure cap policy  
S-097 [S] Daily loss policy  
S-098 [S] Trade frequency policy  
S-099 [S] Cooldown policy  
S-100 [S] Volatility pause policy  
S-101 [S] Stale data policy  
S-102 [S] Drift halt policy  
S-103 [S] Slippage halt policy  
S-104 [S] Circuit breaker policy  
S-105 [S] Safe mode policy  
S-106 [S] Risk reporting contract

### Tests (36)
T-253 [T] Max exposure cap  
T-254 [T] Daily loss cap  
T-255 [T] Trades per day cap  
T-256 [T] Cooldown after loss  
T-257 [T] Volatility pause triggers  
T-258 [T] Stale data blocks  
T-259 [T] Drift halt triggers  
T-260 [T] Slippage halt triggers  
T-261 [T] Circuit breaker triggers  
T-262 [T] Circuit breaker clears  
T-263 [T] Close allowed when halted  
T-264 [T] Close enforced on severe drift  
T-265 [T] Risk guard logs  
T-266 [T] Risk guard returns hold  
T-267 [T] Risk guard returns close  
T-268 [T] Risk guard does not open new positions  
T-269 [T] Guard handles zero equity  
T-270 [T] Guard handles NaN  
T-271 [T] Guard handles negative cash  
T-272 [T] Guard handles price = 0  
T-273 [T] Guard handles missing symbol  
T-274 [T] Guard handles missing aux  
T-275 [T] Safe mode prohibits buy/sell  
T-276 [T] Safe mode allows close  
T-277 [T] Kill file triggers halt  
T-278 [T] Kill file cleared resumes  
T-279 [T] Multiple guards compose  
T-280 [T] Guard order is stable  
T-281 [T] Risk guard deterministic  
T-282 [T] Risk guard backtest parity  
T-283 [T] Guard respects max position size  
T-284 [T] Guard respects max trades per day  
T-285 [T] Guard respects cooldown  
T-286 [T] Guard respects drift severity  
T-287 [T] Guard respects data staleness  
T-288 [T] Risk guard summary output

### Implementation (12)
I-085 [I] Add new risk rules registry  
I-086 [I] Add safe mode state machine  
I-087 [I] Add kill file watcher  
I-088 [I] Add slippage guard  
I-089 [I] Add drift guard  
I-090 [I] Add staleness guard  
I-091 [I] Add risk rule logging  
I-092 [I] Add risk metrics summary  
I-093 [I] Add guard ordering config  
I-094 [I] Add guard tests harness  
I-095 [I] Add guard dry-run mode  
I-096 [I] Add guard telemetry

---

## 7) RECONCILIATION + DRIFT (52)

### Specs (10)
S-107 [S] Reconcile cadence spec  
S-108 [S] Drift thresholds spec  
S-109 [S] Correction policy  
S-110 [S] Reconcile logging policy  
S-111 [S] Balance integrity spec  
S-112 [S] Position integrity spec  
S-113 [S] Order integrity spec  
S-114 [S] Drift severity spec  
S-115 [S] Auto-halt vs auto-correct policy  
S-116 [S] Reconcile report schema

### Tests (32)
T-289 [T] Reconcile detects balance drift  
T-290 [T] Reconcile detects position drift  
T-291 [T] Reconcile detects order drift  
T-292 [T] Reconcile logs drift  
T-293 [T] Drift severity computed  
T-294 [T] Drift triggers halt  
T-295 [T] Reconcile runs on cadence  
T-296 [T] Reconcile handles missing data  
T-297 [T] Reconcile handles API failure  
T-298 [T] Reconcile does not change state silently  
T-299 [T] Drift correction logged  
T-300 [T] Drift correction consistent  
T-301 [T] Drift correction safe mode  
T-302 [T] Reconcile uses correct symbol  
T-303 [T] Reconcile handles multi-strategy  
T-304 [T] Reconcile does not mis-aggregate  
T-305 [T] Reconcile handles stale response  
T-306 [T] Reconcile handles mismatched IDs  
T-307 [T] Drift threshold respects config  
T-308 [T] Reconcile file output  
T-309 [T] Reconcile event count  
T-310 [T] Reconcile still runs if halted  
T-311 [T] Reconcile after restart  
T-312 [T] Reconcile snapshot matches WAL  
T-313 [T] Reconcile positions by side  
T-314 [T] Reconcile cancels stale  
T-315 [T] Reconcile matches open orders  
T-316 [T] Drift record includes state hash  
T-317 [T] Reconcile no-op on match  
T-318 [T] Reconcile includes unknown fields  
T-319 [T] Drift severity logged

### Implementation (10)
I-097 [I] Reconcile worker loop  
I-098 [I] Drift score calculator  
I-099 [I] Reconcile summary emitter  
I-100 [I] Auto-halt on drift  
I-101 [I] Reconcile API adapter  
I-102 [I] Position reconciliation in core  
I-103 [I] Balance reconciliation in core  
I-104 [I] Open orders reconciliation  
I-105 [I] Drift report writer  
I-106 [I] Reconcile CLI tool

---

## 8) DATA INGEST + AUX (72)

### Specs (16)
S-117 [S] Data schema spec  
S-118 [S] Data source priority spec  
S-119 [S] Data gap handling spec  
S-120 [S] Aux feed TTL policy  
S-121 [S] Funding fetch spec  
S-122 [S] Borrow fetch spec  
S-123 [S] Liquidation fetch spec  
S-124 [S] OI fetch spec  
S-125 [S] Depeg detection spec  
S-126 [S] Data cache policy  
S-127 [S] Backoff policy  
S-128 [S] Data validation spec  
S-129 [S] Dataset manifest spec  
S-130 [S] Feed health metrics spec  
S-131 [S] Feed retry policy  
S-132 [S] Aux aggregation spec

### Tests (44)
T-320 [T] Schema validator rejects bad row  
T-321 [T] Schema validator accepts good row  
T-322 [T] Data gaps detected  
T-323 [T] Data gaps logged  
T-324 [T] TTL marks aux stale  
T-325 [T] Backoff increases on errors  
T-326 [T] Backoff caps at max  
T-327 [T] Funding fetch parses correctly  
T-328 [T] Borrow fetch parses correctly  
T-329 [T] Liquidation fetch parses correctly  
T-330 [T] OI fetch parses correctly  
T-331 [T] Depeg calculation correct  
T-332 [T] Aux missing triggers risk block  
T-333 [T] Aux stale triggers risk block  
T-334 [T] Aux fresh allows trade  
T-335 [T] Data cache used when retrying  
T-336 [T] Data cache invalidated on TTL  
T-337 [T] Funding normalization correct  
T-338 [T] Liquidation score aggregation  
T-339 [T] OI change computed  
T-340 [T] Depeg sign correct  
T-341 [T] Feed health metrics  
T-342 [T] Feed error logged  
T-343 [T] Feed retry logged  
T-344 [T] Dataset hash stable  
T-345 [T] Dataset manifest created  
T-346 [T] Data ingestion deterministic  
T-347 [T] Multiple symbol ingestion  
T-348 [T] CSV conversion script output  
T-349 [T] REST paging correctness  
T-350 [T] WS stream parsing  
T-351 [T] Data source fallback  
T-352 [T] Aux default does not mask missing  
T-353 [T] Liquidation window decay  
T-354 [T] Aux fetch time recorded  
T-355 [T] Data ingestion handles 0 volume  
T-356 [T] Data ingestion handles NaN  
T-357 [T] Data ingestion handles outlier  
T-358 [T] Data ingestion handles duplicates  
T-359 [T] Data ingestion logs schema mismatch  
T-360 [T] Aux fetch includes TTL  
T-361 [T] Data ingest runs in paper mode  
T-362 [T] Aux fetch disabled in backtest  
T-363 [T] Data quality report emits summary

### Implementation (12)
I-107 [I] Add data ingestion CLI  
I-108 [I] Add dataset manifest writer  
I-109 [I] Add funding fetcher  
I-110 [I] Add liquidation fetcher  
I-111 [I] Add OI fetcher  
I-112 [I] Add depeg calculator  
I-113 [I] Add aux cache  
I-114 [I] Add TTL enforcement  
I-115 [I] Add feed health monitor  
I-116 [I] Add REST paging support  
I-117 [I] Add WS stream support  
I-118 [I] Add conversion tool for archive data

---

## 9) STRATEGY LIBRARY (64)

### Specs (12)
S-133 [S] Strategy interface spec  
S-134 [S] Strategy config schema  
S-135 [S] Strategy promotion criteria  
S-136 [S] Strategy registry policy  
S-137 [S] Strategy safety guardrails  
S-138 [S] Strategy naming convention  
S-139 [S] Strategy metrics contract  
S-140 [S] Strategy evaluation pipeline  
S-141 [S] Strategy deprecation policy  
S-142 [S] Strategy confidence metric  
S-143 [S] Strategy explainability fields  
S-144 [S] Strategy versioning spec

### Tests (36)
T-364 [T] Strategy registry loads configs  
T-365 [T] Strategy registry rejects invalid  
T-366 [T] Strategy uses aux requirements  
T-367 [T] Strategy blocks on missing aux  
T-368 [T] Strategy stop loss triggers  
T-369 [T] Strategy take profit triggers  
T-370 [T] Strategy time stop triggers  
T-371 [T] Strategy no-trade in unknown regime  
T-372 [T] Strategy respects cooldown  
T-373 [T] Strategy respects max trades  
T-374 [T] Strategy deterministic given seed  
T-375 [T] Strategy config hash stable  
T-376 [T] Strategy evaluation outputs metrics  
T-377 [T] Strategy promotion gate enforced  
T-378 [T] Strategy deprecation guard  
T-379 [T] Strategy safe mode blocked  
T-380 [T] Strategy does not mutate shared state  
T-381 [T] Strategy only pure decisions  
T-382 [T] Strategy reason codes emitted  
T-383 [T] Strategy logs feature digest  
T-384 [T] Strategy logs aux status  
T-385 [T] Strategy performance by regime  
T-386 [T] Strategy outcomes logged  
T-387 [T] Strategy rejects invalid config  
T-388 [T] Strategy trade count matches fills  
T-389 [T] Strategy entry thresholds enforced  
T-390 [T] Strategy exit thresholds enforced  
T-391 [T] Strategy changes captured in manifest  
T-392 [T] Strategy results persist

### Implementation (16)
I-119 [I] Add funding carry strategy  
I-120 [I] Add basis reversion strategy  
I-121 [I] Add depeg snapback strategy  
I-122 [I] Add liquidation momentum strategy  
I-123 [I] Add vol regime switch strategy  
I-124 [I] Add strategy config loader  
I-125 [I] Add strategy registry  
I-126 [I] Add strategy lifecycle states  
I-127 [I] Add strategy versioning  
I-128 [I] Add strategy result exporter  
I-129 [I] Add strategy reason codes  
I-130 [I] Add strategy feature digest  
I-131 [I] Add strategy sensitivity report  
I-132 [I] Add strategy evaluation CLI  
I-133 [I] Add strategy promotion gate  
I-134 [I] Add strategy deprecation gate

---

## 10) OPS + DEPLOY (46)

### Specs (12)
S-145 [S] Deploy profile spec  
S-146 [S] Paper vs live config spec  
S-147 [S] systemd unit spec  
S-148 [S] Log rotation spec  
S-149 [S] Rollback policy  
S-150 [S] Upgrade policy  
S-151 [S] Secrets provisioning spec  
S-152 [S] Monitoring integration spec  
S-153 [S] Incident response policy  
S-154 [S] Operational checklist spec  
S-155 [S] Runbook spec  
S-156 [S] Health check endpoints spec

### Tests (22)
T-393 [T] systemd unit loads  
T-394 [T] Paper config does not trade  
T-395 [T] Live config requires explicit opt-in  
T-396 [T] Log rotation preserves last logs  
T-397 [T] Crash recovery after restart  
T-398 [T] Health endpoint returns OK  
T-399 [T] Health endpoint returns degraded  
T-400 [T] Kill switch from ops works  
T-401 [T] Version mismatch prevents start  
T-402 [T] Secrets not logged  
T-403 [T] Runbook test scenario  
T-404 [T] Monitoring integration ping  
T-405 [T] Alert on fatal  
T-406 [T] Daily summary emitted  
T-407 [T] Run manifest present  
T-408 [T] Config hash logged  
T-409 [T] Ops checklist enforced  
T-410 [T] Upgrade path safe  
T-411 [T] Rollback path safe  
T-412 [T] Disk full handling  
T-413 [T] WAL directory permissions  
T-414 [T] Log directory permissions

### Implementation (12)
I-135 [I] Add systemd unit templates  
I-136 [I] Add deploy scripts  
I-137 [I] Add config validation at boot  
I-138 [I] Add opt-in live guard  
I-139 [I] Add health endpoint  
I-140 [I] Add monitoring hooks  
I-141 [I] Add log rotation  
I-142 [I] Add upgrade script  
I-143 [I] Add rollback script  
I-144 [I] Add run manifest writer  
I-145 [I] Add ops runbook generator  
I-146 [I] Add incident checklist

---

## Total: 672 tickets

---

## Appendix: Current execution status
1. Skeleton loop built + docs entry; deterministic core ready on `skeleton-load-bearing`.
2. Logging overhaul implemented: canonical schema, run manifest, redaction, rollups, and AI brief emitter (see `src/logging.rs` + `docs/SKELETON.md`).
3. WAL/replay tool added (new `docs/SKELETON.md` description) but cross-device build errors still prevent fresh `cargo test`; workaround uses `/tmp` target.
4. Paper backtest run on `data/sample.csv` (`./target/debug/backtest`): strategy churn baseline shows friction losses, p&l -7.31, max dd -42%.
5. Event-driven engine backtest on same dataset (`./target/debug/engine_backtest`): final equity 8538.64, realized pnl -0.36, max dd ~14.6%.
6. Real data fetch script cannot run in this sandbox (DNS blocked); real data awaits outside download (scripts ready).
7. Build/test pipeline still failing across devices due to hardlink errors; use `/tmp` or cached target as workaround.
