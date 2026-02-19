#!/usr/bin/env bash
# Fetch real OHLCV data from Binance public API.
# Usage: ./scripts/fetch_data.sh [symbol] [interval] [limit]
set -euo pipefail

SYMBOL="${1:-BTCUSDT}"
INTERVAL="${2:-1h}"
LIMIT="${3:-1000}"
OUTDIR="$(dirname "$0")/../data"
mkdir -p "$OUTDIR"

OUTFILE="$OUTDIR/${SYMBOL,,}_${INTERVAL}_$(date +%Y%m%d).csv"

echo "Fetching $LIMIT $INTERVAL candles for $SYMBOL..."

# Binance public klines endpoint (no auth required)
URL="https://api.binance.com/api/v3/klines?symbol=${SYMBOL}&interval=${INTERVAL}&limit=${LIMIT}"

RESPONSE=$(curl -sf "$URL")

if [ -z "$RESPONSE" ]; then
    echo "ERROR: empty response from Binance API"
    exit 1
fi

# Parse JSON array of arrays into CSV
# Format: [open_time, open, high, low, close, volume, close_time, ...]
echo "$RESPONSE" | python3 -c "
import json, sys
data = json.load(sys.stdin)
# Header
print('ts,open,high,low,close,volume,funding,borrow,liq,depeg,oi')
for k in data:
    ts = int(k[0]) // 1000  # ms -> s
    o, h, l, c, v = k[1], k[2], k[3], k[4], k[5]
    # Aux fields default to 0 (no funding/borrow/liq/depeg/oi from klines)
    print(f'{ts},{o},{h},{l},{c},{v},0.0,0.0,0.0,0.0,0.0')
" > "$OUTFILE"

ROWS=$(wc -l < "$OUTFILE")
HASH=$(sha256sum "$OUTFILE" | awk '{print $1}')

echo "Written $ROWS lines to $OUTFILE"
echo "SHA256: $HASH"
echo ""
echo "To run backtest: cargo run --release --bin backtest -- $OUTFILE"
