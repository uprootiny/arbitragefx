#!/usr/bin/env bash
set -euo pipefail

# Download Binance spot 1m klines from data.binance.vision (public).
# Usage: ./fetch_binance_klines.sh SYMBOL YEAR START_MONTH END_MONTH OUT_DIR
# Example: ./fetch_binance_klines.sh BTCUSDT 2025 01 03 data/binance

SYMBOL="${1:-}"
YEAR="${2:-}"
START_MONTH="${3:-}"
END_MONTH="${4:-}"
OUT_DIR="${5:-data/binance}"

if [[ -z "$SYMBOL" || -z "$YEAR" || -z "$START_MONTH" || -z "$END_MONTH" ]]; then
  echo "Usage: $0 SYMBOL YEAR START_MONTH END_MONTH OUT_DIR"
  exit 1
fi

mkdir -p "$OUT_DIR"

month="$START_MONTH"
while [[ "$month" -le "$END_MONTH" ]]; do
  mm=$(printf "%02d" "$month")
  file="${SYMBOL}-1m-${YEAR}-${mm}.zip"
  url="https://data.binance.vision/data/spot/monthly/klines/${SYMBOL}/1m/${file}"
  echo "Fetching ${url}"
  curl -fsSL "$url" -o "${OUT_DIR}/${file}"
  month=$((month + 1))
done
