#!/bin/bash
# Fetch historical klines from Binance public API
# Usage: ./fetch_binance_data.sh [symbol] [interval] [limit] [output]

SYMBOL="${1:-BTCUSDT}"
INTERVAL="${2:-5m}"
LIMIT="${3:-1000}"
OUTPUT="${4:-data/btc_binance.csv}"

echo "Fetching $LIMIT $INTERVAL bars for $SYMBOL..."

# Fetch klines
KLINES=$(curl -s "https://api.binance.com/api/v3/klines?symbol=$SYMBOL&interval=$INTERVAL&limit=$LIMIT")

if [ -z "$KLINES" ] || [ "$KLINES" == "[]" ]; then
    echo "Failed to fetch data"
    exit 1
fi

# Fetch funding rate (last entry only - Binance limits this)
FUNDING=$(curl -s "https://fapi.binance.com/fapi/v1/fundingRate?symbol=$SYMBOL&limit=1" 2>/dev/null)
FUNDING_RATE=$(echo "$FUNDING" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d[0]['fundingRate'] if d else '0.0001')" 2>/dev/null || echo "0.0001")

echo "Current funding rate: $FUNDING_RATE"

# Convert to CSV format and write to file
python3 << EOF > "$OUTPUT"
import json

data = json.loads('''$KLINES''')
funding = float('$FUNDING_RATE')

print("ts,o,h,l,c,v,funding,borrow,liq,depeg,oi")

for row in data:
    ts = int(row[0]) // 1000  # Convert ms to seconds
    o = float(row[1])
    h = float(row[2])
    l = float(row[3])
    c = float(row[4])
    v = float(row[5])
    # Use placeholder values for aux data (would need separate API calls)
    borrow = 0.00005
    liq = 0.5
    depeg = 0.0
    oi = 1000000
    print(f'{ts},{o:.2f},{h:.2f},{l:.2f},{c:.2f},{v:.4f},{funding:.8f},{borrow:.8f},{liq:.4f},{depeg:.6f},{int(oi)}')
EOF

echo "Saved to $OUTPUT"
wc -l "$OUTPUT"
