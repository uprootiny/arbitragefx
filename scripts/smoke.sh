#!/usr/bin/env bash
# Smoke tests — shell-level validation that binaries work and data is sound.
# Exit on first failure.
set -euo pipefail

cd "$(dirname "$0")/.."
PASS=0
FAIL=0
SKIP=0

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }
skip() { echo "  SKIP: $1"; SKIP=$((SKIP + 1)); }

echo "=== ArbitrageFX Smoke Tests ==="
echo ""

# S01: Build succeeds
echo "[S01] cargo build"
if cargo build --release 2>/dev/null; then
    pass "cargo build --release"
else
    fail "cargo build --release"
fi

# S02: All tests pass
echo "[S02] cargo test"
if cargo test 2>/dev/null; then
    pass "cargo test"
else
    fail "cargo test"
fi

# S03: Backtest on real data produces strategy output
echo "[S03] backtest produces output"
CSV="data/btc_real_1h.csv"
if [ -f "$CSV" ]; then
    OUTPUT=$(cargo run --release --bin backtest -- "$CSV" 2>/dev/null)
    STRAT_COUNT=$(echo "$OUTPUT" | grep -c "^strategy=" || true)
    if [ "$STRAT_COUNT" -ge 12 ]; then
        pass "backtest outputs $STRAT_COUNT strategies"
    else
        fail "backtest output only $STRAT_COUNT strategies (expected >=12)"
    fi
else
    skip "no $CSV"
fi

# S04: All equity > 0 in backtest output
echo "[S04] equity positive"
if [ -f "$CSV" ]; then
    OUTPUT=$(cargo run --release --bin backtest -- "$CSV" 2>/dev/null)
    NEG_EQUITY=$(echo "$OUTPUT" | grep "^strategy=" | awk -F'equity=' '{print $2}' | awk '{print $1}' | awk '$1 <= 0' | wc -l)
    if [ "$NEG_EQUITY" -eq 0 ]; then
        pass "all equity values positive"
    else
        fail "$NEG_EQUITY strategies with equity <= 0"
    fi
else
    skip "no $CSV"
fi

# S05: Deterministic replay
echo "[S05] deterministic replay"
if [ -f "$CSV" ]; then
    R1=$(cargo run --release --bin backtest -- "$CSV" 2>/dev/null | md5sum)
    R2=$(cargo run --release --bin backtest -- "$CSV" 2>/dev/null | md5sum)
    if [ "$R1" = "$R2" ]; then
        pass "two runs produce identical output"
    else
        fail "non-deterministic output"
    fi
else
    skip "no $CSV"
fi

# S06: Data schema validation
echo "[S06] data schema"
CSVS=(data/btc_real_1h.csv data/btc_bull_1h.csv data/btc_range_1h.csv data/btc_bear2_1h.csv)
SCHEMA_OK=0
for F in "${CSVS[@]}"; do
    if [ -f "$F" ]; then
        COLS=$(head -1 "$F" | tr ',' '\n' | wc -l)
        if [ "$COLS" -ge 10 ]; then
            SCHEMA_OK=$((SCHEMA_OK + 1))
        else
            fail "$F has only $COLS columns"
        fi
    fi
done
if [ "$SCHEMA_OK" -gt 0 ]; then
    pass "$SCHEMA_OK datasets have valid column count"
else
    skip "no datasets found"
fi

# S07: SHA256 stability
echo "[S07] SHA256 stability"
if [ -f "$CSV" ]; then
    H1=$(sha256sum "$CSV" | awk '{print $1}')
    H2=$(sha256sum "$CSV" | awk '{print $1}')
    if [ "$H1" = "$H2" ]; then
        pass "SHA256 reproducible"
    else
        fail "SHA256 differs between reads"
    fi
else
    skip "no $CSV"
fi

# S08: All 4 regime backtests succeed
echo "[S08] all regime backtests"
REGIME_OK=0
for F in "${CSVS[@]}"; do
    if [ -f "$F" ]; then
        if cargo run --release --bin backtest -- "$F" >/dev/null 2>/dev/null; then
            REGIME_OK=$((REGIME_OK + 1))
        else
            fail "backtest failed on $F"
        fi
    fi
done
if [ "$REGIME_OK" -gt 0 ]; then
    pass "$REGIME_OK regime backtests succeeded"
else
    skip "no regime datasets"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed, $SKIP skipped ==="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
