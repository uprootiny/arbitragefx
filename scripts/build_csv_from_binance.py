#!/usr/bin/env python3
import csv
import sys
import zipfile
from pathlib import Path


def iter_rows_from_zip(zpath: Path):
    with zipfile.ZipFile(zpath, "r") as zf:
        for name in zf.namelist():
            if not name.endswith(".csv"):
                continue
            with zf.open(name) as f:
                reader = csv.reader(line.decode("utf-8") for line in f)
                for row in reader:
                    if not row:
                        continue
                    yield row


def main():
    if len(sys.argv) < 3:
        print("Usage: build_csv_from_binance.py IN_DIR OUT_FILE", file=sys.stderr)
        sys.exit(1)
    in_dir = Path(sys.argv[1])
    out_file = Path(sys.argv[2])

    out_file.parent.mkdir(parents=True, exist_ok=True)
    with out_file.open("w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["ts", "open", "high", "low", "close", "volume",
                         "funding", "borrow", "liq", "depeg", "oi"])
        for zpath in sorted(in_dir.glob("*.zip")):
            for row in iter_rows_from_zip(zpath):
                # Binance kline format:
                # 0 open time,1 open,2 high,3 low,4 close,5 volume
                # 6 close time,7 quote vol,8 trades,9 taker buy base,10 taker buy quote,11 ignore
                ts = int(int(row[0]) / 1000)
                writer.writerow([ts, row[1], row[2], row[3], row[4], row[5],
                                 0.0, 0.0, 0.0, 0.0, 0.0])


if __name__ == "__main__":
    main()
