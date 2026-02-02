---- MODULE ledger ----
EXTENDS Naturals

VARIABLES cash, position, equity, price

Init ==
  /\ cash = 1000
  /\ position = 0
  /\ price = 100
  /\ equity = cash + position * price

Fill(q, p, fee) ==
  /\ q \in Integers
  /\ p > 0
  /\ fee >= 0
  /\ cash' = cash - (Abs(q) * p + fee)
  /\ position' = position + q
  /\ price' = p
  /\ equity' = cash' + position' * price'

Next ==
  \E q \in -5..5, p \in 1..1000, fee \in 0..2: Fill(q, p, fee)

Invariant ==
  equity = cash + position * price

Spec ==
  Init /\ [][Next]_<<cash,position,equity,price>>

====
