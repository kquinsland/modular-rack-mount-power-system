# PDCAN Protocol

- Status: executable pre-v1 draft
- CAN mode: CAN-FD with 29-bit extended identifiers
- Network timing: 1 Mbit/s nominal, 2 Mbit/s data phase, BRS enabled
- Byte order: little-endian
- Logical port capacity: eight (`0..=7`)
- Rev A supported-port mask: `0x3f` (ports `0..=5`)

This document describes the layouts implemented by `pdcan-protocol`, the firmware,
`pdcan`, and `pdcan-sim`. Golden-vector tests are the executable compatibility
contract. Values may still change before the explicit v1 freeze after Rev A HIL;
changes must update the codec, tests, this document, and the generated DBC together.

## Operational Identifier

| Bits | Width | Field | Meaning |
| ---: | ---: | --- | --- |
| 28–26 | 3 | Priority | Lower numerical value wins arbitration. |
| 25–22 | 4 | Class | Message category. |
| 21–14 | 8 | Node | `0` broadcast, `1..=254` assigned, `255` invalid. |
| 13–10 | 4 | Target | Port `0..=7`, all ports `14`, board `15`. |
| 9–6 | 4 | Requester | Request/response route; `0` is protocol-reserved. |
| 5–0 | 6 | Opcode | Up to 64 operations per class. |

```text
id = priority << 26
   | class << 22
   | node << 14
   | target << 10
   | requester << 6
   | opcode
```

Requester IDs are diagnostic/routing identities, not authentication:

| ID | Use |
| ---: | --- |
| `0` | Protocol-generated, unscoped traffic. |
| `1..=14` | Configured embedded or host requesters. |
| `15` | Default development `pdcan` requester. |

Simultaneously active command hosts must use different requester IDs. `pdcan
monitor` shows all requester traffic by default; filtering is optional.

## Commissioning Identifier

Commissioning keeps priority, class, requester, and opcode but reuses the
operational Node and Target bits as one 12-bit arbitration token:

| Bits | Width | Field |
| ---: | ---: | --- |
| 28–26 | 3 | Priority (`6`) |
| 25–22 | 4 | Commissioning class (`0xe`) |
| 21–10 | 12 | UID-derived token |
| 9–6 | 4 | Requester |
| 5–0 | 6 | Commissioning opcode |

The token starts with reflected IEEE CRC-32 of the full 12-byte UID, XORs in both
halves of the 64-bit nonce (with rotation), applies the Murmur3 32-bit avalanche
finalizer, and takes the low 12 bits. It is only an arbitration aid; the full
96-bit factory UID in the payload is authoritative. The nonlinear nonce-keyed
finalizer is required: simply truncating `CRC32(UID || nonce)` leaves pairwise CRC
collisions invariant under nonce rotation. A test constructs that failure class
and proves the implemented mixer separates it. `NODE_CLAIM` uses a domain-separated
nonce containing the Node ID and claim round.

See [commissioning.md](commissioning.md) for nonce rotation, claim windows, and
conflict recovery.

## Classes and Priorities

| Class | Value | Purpose | Priority |
| --- | ---: | --- | ---: |
| Control | `0x0` | Desired-state and emergency commands | `0` or `1` |
| Response | `0x1` | Correlated command outcomes | `2` |
| State | `0x2` | State changes and requested/periodic snapshots | `3` |
| Telemetry | `0x3` | Freshness-oriented measurements | `4` |
| Management | `0x4` | Heartbeat, identity, health, and versions | `5` |
| Configuration | `0x5` | Reserved persistent-configuration expansion | — |
| Commissioning | `0xe` | UID provisioning and conflict recovery | `6` |
| Debug | `0xf` | Development-only traffic | `7` |

All fields use integer units: millivolts, milliamps, milliwatts, signed
centi-degrees Celsius, and explicitly named seconds. Transmitters set reserved
bytes to zero. The pre-v1 exact-version decoder rejects nonzero reserved fields,
invalid enum values, wrong priority/target, and non-exact payload lengths.

## Request Correlation and Retries

Every mutating or status command carries a 64-bit `RequestId` at bytes `0..8`.
Every result echoes it and retains the requester in the arbitration ID. `pdcan`
retries with the same requester, request ID, opcode, target, and payload.

Firmware retains a bounded completed-request cache. An exact retry replays the
completed response. Reuse of one `(RequesterId, RequestId)` for different command
content is rejected as `INVALID_ARGUMENT`. Pending duplicates do not apply the
mutation again. The rule also applies to UID-addressed commissioning mutations.
The 64-bit space and bounded cache replace a permanent 16-bit transaction-ID
interpretation.

Ordinary persistent mutations are serialized: a second mutation receives the
transient `BUSY` result until the first is durable. `EMERGENCY_DISABLE` is the
exception and may preempt a pending mutation. This prevents one coalesced flash
snapshot from falsely acknowledging two mutually superseding requested values.

## Operational Message Allocation

| Class | Opcode | Name | Target | Length | Sender |
| --- | ---: | --- | --- | ---: | --- |
| Control | `0` | `EMERGENCY_DISABLE` | all ports | 8 | host |
| Control | `1` | `SET_PORT_POLICY` | port | 20 | host |
| Control | `2` | `ACKNOWLEDGE_EMERGENCY_RESOLVED` | board | 8 | host |
| Control | `3` | `REQUEST_STATUS` | port | 8 | host |
| Control | `4` | `SET_FAN_CONFIG` | board | 12 | host |
| Response | `0` | `COMMAND_RESPONSE` | board | 16 | backpack |
| State | `0` | `PORT_STATE` | port | 16 | backpack |
| Telemetry | `0` | `PORT_POWER` | port | 16 | backpack |
| Telemetry | `1` | `BOARD_TEMPERATURE` | board | 4 | backpack |
| Management | `0` | `HEARTBEAT` | board | 24 | backpack |
| Management | `1` | `BOARD_INFO` | board | 24 | backpack |

### Simple Control Payloads

`EMERGENCY_DISABLE`, `ACKNOWLEDGE_EMERGENCY_RESOLVED`, and `REQUEST_STATUS` are
exactly one field:

| Bytes | Type | Field |
| --- | --- | --- |
| `0..8` | `u64` | `RequestId` |

The identifier carries the board/port target. Emergency may use Node `0` for all
commissioned and uncommissioned backpacks. Uncommissioned/conflicted boards obey
it but suppress collision-prone operational responses.

### `SET_PORT_POLICY`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..8` | `u64` | `RequestId` |
| `8` | flags | bit 0 `enabled`; all others zero |
| `9` | — | reserved zero |
| `10..12` | `u16` | maximum voltage, mV |
| `12..14` | `u16` | maximum current, mA |
| `14..16` | — | reserved zero |
| `16..20` | `u32` | maximum power, mW |

Firmware validates the persisted desired policy against hard caps of 20,000 mV,
5,000 mA, and 100,000 mW for every logical slot. EPR and the proprietary 7 A mode
are not representable. Rev A rejects targets 6 and 7 as unsupported rather than
treating them as empty.

`OK_PENDING` means the policy is durable and queued/applied according to module
presence; it does not claim that USB-PD renegotiation has completed.

### `SET_FAN_CONFIG`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..8` | `u64` | `RequestId` |
| `8` | `u8` | mode: `0` 3-wire, `1` 4-wire |
| `9` | `u8` | requested duty, `0..=100` percent |
| `10..12` | — | reserved zero |

Mode and requested duty are persistent. Reset/fault handling may override them to
full speed without changing the stored request.

### `COMMAND_RESPONSE`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..8` | `u64` | echoed `RequestId` |
| `8` | `u8` | request opcode |
| `9` | `u8` | request target |
| `10` | `u8` | result code |
| `11` | — | reserved zero |
| `12..16` | `u32` | result-specific detail, zero unless documented |

| Code | Result | Code | Result |
| ---: | --- | ---: | --- |
| `0` | `OK` | `8` | `I2C_ERROR` |
| `1` | `OK_PENDING` | `9` | `PD_CONTROLLER_ERROR` |
| `2` | `INVALID_ARGUMENT` | `10` | `TIMEOUT` |
| `3` | `INVALID_TARGET` | `11` | `VERIFY_FAILED` |
| `4` | `UNSUPPORTED` | `12` | `ADDRESS_CONFLICT` |
| `5` | `BUSY` | `13` | `PERSIST_FAILED` |
| `6` | `NO_MODULE` | `14` | `EMERGENCY_LATCHED` |
| `7` | `NOT_READY` | `15` | `INTERNAL_ERROR` |

Success for a persistent mutation is emitted only after a successful flash
completion whose revision durably covers that request. Failed writes remain
pending and retry; they do not prematurely acknowledge an emergency clear. If a
new emergency arrives while a clear is in flight, the clear receives
`EMERGENCY_LATCHED` and the emergency request is not acknowledged until the
re-latched state is durable.

### `PORT_STATE`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..2` | `u16` | snapshot sequence |
| `2..4` | flags | state flags below |
| `4..8` | `u32` | fault flags |
| `8..12` | `u32` | uptime, seconds |
| `12` | `u8` | active PDO/profile; `255` unknown/none |
| `13` | `u8` | low byte of slot generation |
| `14..16` | — | reserved zero |

| Bit | Flag |
| ---: | --- |
| `0` | enabled desired policy |
| `1` | module present |
| `2` | policy pending |
| `3` | emergency latched |
| `4` | USB connected |
| `5` | contract valid |
| `6` | module input power successfully commanded on (not rail readback) |

The current firmware populates desired enable, module presence, policy-pending,
emergency state, confirmed commanded input-power state, monotonic uptime, and generation.
Connection, contract, active profile, and full fault observation await verified
SW3538 semantics. On boards without a controllable input-power switch, every
supported port reports input power enabled. Rev B has no FET/rail readback, so bit
6 does not prove the physical rail state after an I2C or expander fault.

### `PORT_POWER`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..2` | `u16` | sample sequence |
| `2..4` | `u16` | measured voltage, mV |
| `4..6` | `u16` | measured current, mA |
| `6..10` | `u32` | measured power, mW |
| `10..12` | `i16` | temperature, centi-degrees Celsius |
| `12..14` | `u16` | negotiated contract voltage, mV |
| `14..16` | `u16` | negotiated contract current, mA |

The codec and golden vectors exist; physical telemetry emission remains blocked
on the one-port SW3538 validation spike.

### `BOARD_TEMPERATURE`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..2` | `u16` | successful sample sequence |
| `2..4` | `i16` | PCB temperature, centi-degrees Celsius |

Rev B reads the backplane TMP102 at approximately 1 Hz from the direct upstream
I2C bus. Firmware emits only successful samples; it does not configure or use
the TMP102 alert and threshold registers. The sample sequence wraps naturally
and increments only after a successful read.

### `HEARTBEAT`

| Byte(s) | Type | Field |
| --- | --- | --- |
| `0`, `1` | `u8` | protocol major, minor |
| `2..4`, `4..6`, `6..8` | `u16` | firmware major, minor, patch |
| `8` | `u8` | hardware revision |
| `9` | flags | health flags |
| `10` | bitmap | supported ports |
| `11` | bitmap | online modules |
| `12` | bitmap | enabled desired policies |
| `13` | bitmap | faulted ports |
| `14` | `0/1` | emergency latched |
| `15` | enum | commissioning state |
| `16` | flags | boot reset cause (table below) |
| `17..20` | — | reserved zero |
| `20..24` | `u32` | uptime, seconds |

Reset-cause flags mirror the STM32 RCC sticky indicators captured before firmware
clears them:

| Bit | Cause |
| ---: | --- |
| `0` | option-byte loader |
| `1` | reset pin |
| `2` | brownout or power-on/power-down reset |
| `3` | software reset |
| `4` | independent watchdog |
| `5` | window watchdog |
| `6` | illegal low-power-mode entry |

Commissioned, conflict-free firmware emits this approximately once per second.

### `BOARD_INFO`

| Bytes | Type | Field |
| --- | --- | --- |
| `0..12` | bytes | full STM32 UID |
| `12` | `u8` | Node ID, matching arbitration ID |
| `13` | enum | commissioning state |
| `14` | `u8` | hardware revision |
| `15` | bitmap | supported ports |
| `16..22` | three `u16` | firmware major, minor, patch |
| `22`, `23` | `u8` | protocol major, minor |

Discovery is the version-tolerant identity path; `BOARD_INFO` is an operational
snapshot for compatible commissioned nodes.

## Commissioning Messages

| Opcode | Name | Length | Sender | Has `RequestId` |
| ---: | --- | ---: | --- | --- |
| `0` | `NODE_CLAIM` | 16 | backpack | no |
| `1` | `DISCOVER` | 8 | host | no (discovery nonce) |
| `2` | `DISCOVERY_RESPONSE` | 32 | backpack | no |
| `3` | `IDENTIFY` | 24 | host | yes |
| `4` | `ASSIGN_NODE` | 24 | host | yes |
| `5` | `CLEAR_NODE` | 20 | host | yes |
| `6` | `COMMISSIONING_RESULT` | 24 | backpack | yes |

Commissioning state values are `0` uncommissioned, `1` claiming, `2`
commissioned, and `3` address conflict.

### `NODE_CLAIM`

| Bytes | Field |
| --- | --- |
| `0..12` | full UID |
| `12` | claimed Node ID |
| `13` | commissioning state |
| `14..16` | claim-round nonce (`u16`) |

### Discovery

`DISCOVER` is one `u64` nonce. `DISCOVERY_RESPONSE` is:

| Bytes | Type | Field |
| --- | --- | --- |
| `0..12` | bytes | full UID |
| `12` | `u8` | Node ID; `0` means uncommissioned |
| `13` | enum | commissioning state |
| `14`, `15` | `u8` | protocol major, minor |
| `16..22` | three `u16` | firmware major, minor, patch |
| `22` | `u8` | hardware revision |
| `23` | bitmap | supported ports |
| `24..32` | `u64` | echoed discovery nonce |

### UID-Addressed Commands

`IDENTIFY`:

| Bytes | Field |
| --- | --- |
| `0..8` | `RequestId` |
| `8..20` | UID |
| `20..22` | duration, seconds; zero stops |
| `22..24` | reserved zero |

`ASSIGN_NODE` uses `RequestId`, UID, Node ID at byte 20, then three reserved zero
bytes. `CLEAR_NODE` contains only `RequestId` and UID.

`COMMISSIONING_RESULT`:

| Bytes | Field |
| --- | --- |
| `0..8` | echoed `RequestId` |
| `8..20` | UID |
| `20` | request opcode |
| `21` | result code |
| `22` | current Node ID; zero means none |
| `23` | commissioning state |

These operations remain UID-addressable when uncommissioned or in
`AddressConflict`. Assignment/clear success waits for durable persistence.

## Compatibility and Update Boundary

The current operational policy requires an exact protocol major/minor match.
Discovery remains independently decodable enough to report UID, hardware,
firmware, protocol, and incompatibility. An incompatible CLI must not attempt
operational control.

CAN firmware update is outside v1. SWD/service tooling is the update path. A future
bootloader needs a separately stable management namespace; no updater, image slot,
or update command is allocated here.

## Emergency Disable

`EMERGENCY_DISABLE` is the highest-priority best-effort Rev A command. Runtime
latching is immediate. Success is sent only after the latched record is durable.
The latch survives watchdog reset and complete power loss, blocks enabling
operations, and clears only through `ACKNOWLEDGE_EMERGENCY_RESOLVED` after the
operator declares the danger resolved. Clear success also waits for durability.

Rev A shutdown uses the ordinary I2C/SW3538 path and is not a safety-rated
independent cutoff. It cannot guarantee the 100 W policy during the controller's
pre-configuration power-on interval because Rev A lacks a high-side disconnect.

Rev B sends an all-zero output-register write to the upstream PCA9554 before any
SW3538 cleanup. This cuts module input power independently of SW3538
responsiveness, but still depends on a functioning upstream I2C bus. The PCA9554
has no asynchronous output-enable or reset input, so MCU-only reset can also leave
previously enabled ports on until startup firmware successfully writes all-off.
Because each FET powers down the SW3538 itself, a shorter default-state interval
still exists after an individual port is powered and before its stored policy can
be applied. No numeric or safety-rated shutdown guarantee is claimed before HIL
measurement.

## Generated DBC

Run:

```text
cargo xtask dbc
cargo xtask dbc --check
```

[`can/pdcan.dbc`](can/pdcan.dbc) is generated from
`pdcan_protocol::MESSAGE_DEFINITIONS`. Operational entries use representative
Node/target/requester values; commissioning entries use representative token 1.
The Rust codec and golden vectors remain authoritative for dynamic-ID semantics
and strict payload validation.
