---- MODULE order_sm ----
EXTENDS Naturals, Sequences

CONSTANTS MaxQty

VARIABLES state, filledQty

States == {"New","Submitted","Acked","PartiallyFilled","Filled","Canceled","Rejected"}

Init ==
  /\ state = "New"
  /\ filledQty = 0

Submit ==
  /\ state = "New"
  /\ state' = "Submitted"
  /\ UNCHANGED filledQty

Ack ==
  /\ state = "Submitted"
  /\ state' = "Acked"
  /\ UNCHANGED filledQty

Fill(q) ==
  /\ state \in {"Acked","PartiallyFilled"}
  /\ q > 0
  /\ filledQty + q <= MaxQty
  /\ filledQty' = filledQty + q
  /\ state' = IF filledQty' = MaxQty THEN "Filled" ELSE "PartiallyFilled"

Cancel ==
  /\ state \in {"Acked","PartiallyFilled","Submitted"}
  /\ state' = "Canceled"
  /\ UNCHANGED filledQty

Reject ==
  /\ state \in {"Submitted","Acked","PartiallyFilled"}
  /\ state' = "Rejected"
  /\ UNCHANGED filledQty

Next ==
  Submit \/ Ack \/ Cancel \/ Reject \/ (\E q \in 1..MaxQty: Fill(q))

Invariant ==
  /\ filledQty <= MaxQty
  /\ state = "Filled" => filledQty = MaxQty
  /\ state = "Canceled" => filledQty < MaxQty

Spec ==
  Init /\ [][Next]_<<state,filledQty>>

==== 
