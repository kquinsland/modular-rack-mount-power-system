# PDCAN Commissioning

- Status: executable pre-v1 draft
- Permanent identity: STM32 factory 96-bit UID
- Installation identity: flash-persisted Node ID `1..=254`

The silicon value is called a UID, not a UUID. It has no assumed UUID variant or
version semantics. UID-addressed operations remain available before assignment,
after assignment, and during an address conflict.

## Operator Workflow

```text
pdcan scan --interface can0
pdcan identify <24-hex-digit-uid> --seconds 30
# Operator observes the selected backpack.
pdcan assign <uid> 3
pdcan scan
```

`pdcan assign` scans first and refuses an address reported by another UID. `--force`
bypasses only that host-side occupancy check; it does not disable firmware conflict
detection and should be used only after the operator understands the conflict.

Clear an assignment without relying on its Node ID:

```text
pdcan clear-node <uid>
```

Assignment and clear results are emitted only after the updated flash record is
durable. A reboot is not required.

## States

```text
Uncommissioned -> Claiming -> Commissioned
                         \-> AddressConflict
Commissioned ------------> AddressConflict
AddressConflict --assign/clear by UID--> Claiming/Uncommissioned
```

- `Uncommissioned`: discovery and UID operations only; no ordinary Node-ID traffic.
- `Claiming`: a Node ID exists, but the startup claim window is not complete.
- `Commissioned`: ordinary traffic is allowed.
- `AddressConflict`: the persisted Node ID is retained for evidence, ordinary
  traffic is suppressed, and UID recovery remains available.

Firmware never auto-erases a conflicting address. An operator explicitly assigns
or clears one or both boards.

## Multi-Round Discovery

For each round, `pdcan` sends `DISCOVER(nonce)`. Each board:

1. computes CRC-32 of the UID and applies the specified nonlinear nonce-keyed
   avalanche mixer;
2. places that token in the commissioning arbitration ID;
3. includes the full UID and echoed nonce in its response; and
4. responds only to the requesting `RequesterId` route.

The host listens for a bounded interval, rotates the nonce, and merges results by
full UID across rounds. The default is three rounds. A 12-bit collision may lose
both physical CAN responses for one nonce, but a new nonce normally separates the
pair; a golden/property test constructs and verifies this case. The CLI allows up
to 16 rounds when diagnosing an unusually persistent collision.

Linux `vcan` cannot reproduce destructive same-ID/different-data arbitration, so
the physical collision/error-frame behavior and timeout margin remain HIL items.

## Duplicate Node Claims

At startup and after assignment, a board emits claim rounds with nonces 0, 1, and
2 while suppressing normal traffic for the provisional 500 ms claim window. The
payload contains full UID, claimed Node ID, state, and round nonce. Nonce-dependent
tokens allow one round to recover when two UIDs share a 12-bit token in another.

If a board sees a different UID claim its persisted Node ID, it enters
`AddressConflict` and sends one claim response containing its own UID and conflict
state and the next claim nonce. This response lets a newly attached conflicting
board observe the existing board too, without an endless claim-response loop. Both
then suppress normal Node-ID traffic.

The 500 ms window is a draft software value. Multiple-node HIL must validate it
against physical arbitration, bus load, startup skew, and adapters before v1.

## Identify

`pdcan identify UID` requests a white/off blink overlay for 30 seconds by default.
The duration is bounded by a `u16` seconds field; zero stops the overlay. Base
health state continues to update beneath the overlay. Identify never changes port
policy, PD negotiation, fan behavior, Node ID, or persistence.

The TIM15/DMA waveform and visible pattern still require Rev A validation.

## Request and Retry Rules

`IDENTIFY`, `ASSIGN_NODE`, and `CLEAR_NODE` use the same 64-bit request correlation
as operational commands. A retry keeps the same requester, request ID, UID, opcode,
and content. Exact pending retries wait for the original flash completion; exact
completed retries replay the cached result. Reusing an identity with different
content is rejected. Discovery uses its nonce rather than a `RequestId`.

Requester ID 0 is reserved for protocol-generated claims. The default `pdcan`
requester is 15; use `--requester` when another host is active.

## Emergency Interaction

Broadcast emergency disable is obeyed even while a board is uncommissioned,
claiming, or conflicted. Such a board suppresses an operational response when it
cannot provide a collision-safe commissioned Node ID. The durable emergency latch
is still visible in later discovery/operation and requires explicit operator
acknowledgement after commissioning/conflict recovery.
