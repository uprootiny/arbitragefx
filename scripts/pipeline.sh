#!/usr/bin/env bash
# Pipeline: fetch -> validate -> backtest -> report
# Single command to run the full ingestion/backtest loop.
set -euo pipefail

cd "$(dirname "$0")/.."
REPORT_DIR="out/reports"
mkdir -p "$REPORT_DIR"

DATE=$(date +%Y-%m-%d)
REPORT="$REPORT_DIR/$DATE.txt"

echo "=== ArbitrageFX Pipeline — $DATE ===" | tee "$REPORT"
echo "" | tee -a "$REPORT"

# Step 1: Validate existing data
echo "--- Step 1: Data Validation ---" | tee -a "$REPORT"
CSVS=(data/btc_real_1h.csv data/btc_bull_1h.csv data/btc_range_1h.csv data/btc_bear2_1h.csv)
for CSV in "${CSVS[@]}"; do
    if [ -f "$CSV" ]; then
        ROWS=$(wc -l < "$CSV")
        HASH=$(sha256sum "$CSV" | awk '{print $1}')
        echo "  $CSV: $ROWS rows, sha256=${HASH:0:16}..." | tee -a "$REPORT"
    else
        echo "  $CSV: MISSING" | tee -a "$REPORT"
    fi
done
echo "" | tee -a "$REPORT"

# Step 2: Run backtests on all regime datasets
echo "--- Step 2: Backtests ---" | tee -a "$REPORT"
for CSV in "${CSVS[@]}"; do
    if [ ! -f "$CSV" ]; then
        echo "  SKIP $CSV (not found)" | tee -a "$REPORT"
        continue
    fi
    BASENAME=$(basename "$CSV" .csv)
    echo "  Running $BASENAME..." | tee -a "$REPORT"
    OUTPUT=$(cargo run --release --bin backtest -- "$CSV" 2>/dev/null || echo "BACKTEST_FAILED")
    if echo "$OUTPUT" | grep -q "BACKTEST_FAILED"; then
        echo "    FAILED" | tee -a "$REPORT"
        continue
    fi
    # Extract baselines
    BH=$(echo "$OUTPUT" | grep "^baseline=buy_hold" | awk -F'pnl=' '{print $2}')
    echo "    buy_hold=$BH" | tee -a "$REPORT"

    # Extract best and worst strategy by equity_pnl
    BEST=$(echo "$OUTPUT" | grep "^strategy=" | sort -t= -k3 -n -r | head -1)
    WORST=$(echo "$OUTPUT" | grep "^strategy=" | sort -t= -k3 -n | head -1)
    BEST_ID=$(echo "$BEST" | awk -F'[ =]' '{print $2}')
    BEST_PNL=$(echo "$BEST" | grep -oP 'equity_pnl=[\-0-9.]+' | cut -d= -f2)
    WORST_ID=$(echo "$WORST" | awk -F'[ =]' '{print $2}')
    WORST_PNL=$(echo "$WORST" | grep -oP 'equity_pnl=[\-0-9.]+' | cut -d= -f2)
    echo "    best: $BEST_ID equity_pnl=$BEST_PNL" | tee -a "$REPORT"
    echo "    worst: $WORST_ID equity_pnl=$WORST_PNL" | tee -a "$REPORT"

    # Count net-positive strategies
    POS_COUNT=$(echo "$OUTPUT" | grep "^strategy=" | grep -oP 'equity_pnl=[\-0-9.]+' | cut -d= -f2 | awk '$1 > 0' | wc -l)
    TOTAL=$(echo "$OUTPUT" | grep -c "^strategy=")
    echo "    net_positive: $POS_COUNT/$TOTAL" | tee -a "$REPORT"
    echo "" | tee -a "$REPORT"
done

# Step 3: Summary
echo "--- Step 3: Summary ---" | tee -a "$REPORT"
echo "  Report written to: $REPORT" | tee -a "$REPORT"
echo "  Timestamp: $(date -Iseconds)" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"
echo "Done."
