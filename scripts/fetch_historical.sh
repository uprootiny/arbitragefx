#!/bin/bash
# Fetch extended historical data from Binance (paginated)
# Usage: ./fetch_historical.sh [symbol] [interval] [days] [output]

SYMBOL="${1:-BTCUSDT}"
INTERVAL="${2:-5m}"
DAYS="${3:-180}"
OUTPUT="${4:-data/${SYMBOL,,}_${INTERVAL}_${DAYS}d.csv}"

# Calculate bars needed
case $INTERVAL in
    1m)  BARS_PER_DAY=1440 ;;
    5m)  BARS_PER_DAY=288 ;;
    15m) BARS_PER_DAY=96 ;;
    1h)  BARS_PER_DAY=24 ;;
    4h)  BARS_PER_DAY=6 ;;
    1d)  BARS_PER_DAY=1 ;;
    *)   BARS_PER_DAY=288 ;;
esac

TOTAL_BARS=$((DAYS * BARS_PER_DAY))
BATCH_SIZE=1000
BATCHES=$(( (TOTAL_BARS + BATCH_SIZE - 1) / BATCH_SIZE ))

echo "Fetching $TOTAL_BARS bars ($DAYS days of $INTERVAL) for $SYMBOL"
echo "Batches needed: $BATCHES"

# Get current time in ms
END_TIME=$(date +%s)000

# Write header
echo "ts,o,h,l,c,v,funding,borrow,liq,depeg,oi" > "$OUTPUT"

FETCHED=0
for ((i=BATCHES-1; i>=0; i--)); do
    # Calculate end time for this batch
    BATCH_END=$END_TIME

    # Fetch batch
    KLINES=$(curl -s "https://api.binance.com/api/v3/klines?symbol=$SYMBOL&interval=$INTERVAL&limit=$BATCH_SIZE&endTime=$BATCH_END")

    if [ -z "$KLINES" ] || [ "$KLINES" == "[]" ]; then
        echo "Batch $((BATCHES-i))/$BATCHES: No data"
        break
    fi

    # Parse and append
    python3 << EOF >> "$OUTPUT"
import json
data = json.loads('''$KLINES''')
for row in data:
    ts = int(row[0]) // 1000
    o, h, l, c, v = float(row[1]), float(row[2]), float(row[3]), float(row[4]), float(row[5])
    print(f'{ts},{o:.2f},{h:.2f},{l:.2f},{c:.2f},{v:.4f},0.0001,0.00005,0.5,0.0,1000000')
EOF

    # Update end time for next batch (oldest timestamp - 1)
    OLDEST=$(echo "$KLINES" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d[0][0] if d else 0)")
    END_TIME=$((OLDEST - 1))

    BATCH_COUNT=$(echo "$KLINES" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")
    FETCHED=$((FETCHED + BATCH_COUNT))

    echo "Batch $((BATCHES-i))/$BATCHES: +$BATCH_COUNT bars (total: $FETCHED)"

    # Rate limit
    sleep 0.2
done

# Sort by timestamp and remove duplicates
sort -t',' -k1 -n "$OUTPUT" | uniq > "${OUTPUT}.tmp"
mv "${OUTPUT}.tmp" "$OUTPUT"

FINAL_COUNT=$(wc -l < "$OUTPUT")
echo ""
echo "Saved $((FINAL_COUNT-1)) bars to $OUTPUT"
head -3 "$OUTPUT"
echo "..."
tail -2 "$OUTPUT"
