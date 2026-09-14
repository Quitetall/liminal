---- MODULE ToolControl ----
EXTENDS Naturals

VARIABLES intent, effect, ack, committed

Vars == <<intent, effect, ack, committed>>

Init ==
    /\ intent = FALSE
    /\ effect = FALSE
    /\ ack = FALSE
    /\ committed = FALSE

Prepare ==
    /\ ~intent
    /\ intent' = TRUE
    /\ UNCHANGED <<effect, ack, committed>>

Apply ==
    /\ intent
    /\ ~effect
    /\ effect' = TRUE
    /\ UNCHANGED <<intent, ack, committed>>

Ack ==
    /\ effect
    /\ ~ack
    /\ ack' = TRUE
    /\ UNCHANGED <<intent, effect, committed>>

Finalize ==
    /\ ack
    /\ ~committed
    /\ committed' = TRUE
    /\ UNCHANGED <<intent, effect, ack>>

\* Explicit stuttering is permitted; this bootstrap makes no liveness claim.
Next == Prepare \/ Apply \/ Ack \/ Finalize \/ UNCHANGED Vars

CommittedImpliesAck == committed => ack
AckImpliesEffect == ack => effect
EffectImpliesIntent == effect => intent

====
