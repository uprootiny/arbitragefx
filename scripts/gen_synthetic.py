#!/usr/bin/env python3
"""Generate synthetic BTC price data for backtesting.

Creates realistic-looking price series with:
- Mean-reverting behavior (Ornstein-Uhlenbeck)
- Occasional regime changes
- Correlated funding/aux data
"""

import random
import math
import sys

def ou_step(x, mu, theta, sigma, dt=1.0):
    """Ornstein-Uhlenbeck mean-reverting step."""
    dx = theta * (mu - x) * dt + sigma * math.sqrt(dt) * random.gauss(0, 1)
    return x + dx

def main():
    n_bars = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
    start_ts = 1704067200  # 2024-01-01 00:00:00 UTC
    bar_secs = 300  # 5-minute bars

    # Initial state
    price = 42000.0
    vol_base = 150.0
    funding = 0.0001
    oi = 1_000_000

    # OU parameters
    price_mu = 42500
    price_theta = 0.002  # slow mean reversion
    price_sigma = 80.0

    funding_mu = 0.00008
    funding_theta = 0.1
    funding_sigma = 0.00003

    print("ts,o,h,l,c,v,funding,borrow,liq,depeg,oi")

    for i in range(n_bars):
        ts = start_ts + i * bar_secs

        # Price evolution (OU + momentum bursts)
        if random.random() < 0.02:  # 2% chance of regime shift
            price_mu = price + random.gauss(0, 500)

        o = price

        # Intrabar volatility
        bar_vol = vol_base * (0.5 + random.random())
        c = ou_step(price, price_mu, price_theta, price_sigma)
        c = max(c, 1000)  # floor

        h = max(o, c) + abs(random.gauss(0, bar_vol * 0.5))
        l = min(o, c) - abs(random.gauss(0, bar_vol * 0.5))

        # Volume correlated with price change
        v = 1000 + abs(c - o) * 10 + random.gauss(0, 200)
        v = max(v, 100)

        # Funding (OU)
        funding = ou_step(funding, funding_mu, funding_theta, funding_sigma)
        funding = max(-0.003, min(0.003, funding))  # clamp

        # Borrow rate (lower than funding usually)
        borrow = abs(funding) * 0.3 + random.gauss(0, 0.00002)
        borrow = max(0, borrow)

        # Liquidation score (spikes during big moves)
        liq = 0.5 + abs(c - o) / vol_base + random.gauss(0, 0.3)
        liq = max(0, liq)

        # Depeg (usually near zero)
        depeg = random.gauss(0, 0.0003)
        depeg = max(-0.01, min(0.01, depeg))

        # Open interest (random walk)
        oi = oi * (1 + random.gauss(0, 0.01))
        oi = max(100000, oi)

        print(f"{ts},{o:.2f},{h:.2f},{l:.2f},{c:.2f},{v:.1f},{funding:.8f},{borrow:.8f},{liq:.4f},{depeg:.6f},{int(oi)}")

        price = c

if __name__ == "__main__":
    main()
