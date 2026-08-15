# PDCAN Protocol

- Status: pre-v1 draft
- CAN mode: CAN-FD with 29-bit extended identifiers
- Network timing: 1 Mbit/s nominal, 2 Mbit/s data phase, BRS enabled
- Byte order: little-endian

This document records the executable protocol skeleton. Exact operational payloads
remain draft until the SW3538 capability spike and Rev A HIL validation. The Rust
definitions and golden vectors in `pdcan-protocol` are the source for generated
artifacts.

## Identifier Layout

| Bits | Width | Field | Notes |
| ---: | ---: | --- | --- |
| 28–26 | 3 | Priority | Lower numerical value wins arbitration. |
| 25–22 | 4 | Class | Message category; allocations are below. |
| 21–14 | 8 | Node | `0` broadcast, `1..=254` assigned. |
| 13–10 | 4 | Target | Ports `0..=7`, all ports `14`, board `15`. |
| 9–6 | 4 | Requester | Host identity; `0` is unscoped. |
| 5–0 | 6 | Opcode | Up to 64 operations per class. |

```text
id = priority << 26
   | class << 22
   | node << 14
   | target << 10
   | requester << 6
   | opcode
```

The requester field intentionally reduces the former ten-bit message-type field
to a four-bit requester and six-bit opcode. The allocation supports sixteen
requester values and 64 opcodes per class without increasing payload overhead.

## Request Identity

Requester ID allocation for the draft is:

| ID | Use |
| ---: | --- |
| `0` | Reserved for unscoped protocol traffic. |
| `1..=14` | Configured embedded or host requesters. |
| `15` | Default development `pdcan` requester. |

Requester IDs route and diagnose traffic; they are not authentication identities.
Simultaneously active hosts must not intentionally use the same requester ID.

Every command starts with a 64-bit `RequestId`, encoded little-endian, and every
response echoes the requester and request ID. Deduplication also includes opcode,
target, command identity/digest, and a bounded retention lifetime.

`pdcan monitor` displays all requester traffic by default. `--requests` restricts
the view to control requests and `--requester ID` applies an optional diagnostic
filter.

## Classes and Priorities

| Class | Value | Purpose |
| --- | ---: | --- |
| Control | `0x0` | Desired-state and emergency commands. |
| Response | `0x1` | Correlated command outcomes. |
| State | `0x2` | State changes and periodic snapshots. |
| Telemetry | `0x3` | Freshness-oriented measurements. |
| Management | `0x4` | Heartbeat, board identity, health, and versions. |
| Configuration | `0x5` | Persistent configuration operations. |
| Commissioning | `0xE` | UID commissioning and conflict recovery. |
| Debug | `0xF` | Development-only traffic. |

| Priority | Purpose |
| ---: | --- |
| `0` | Emergency/critical fault. |
| `1` | Control command. |
| `2` | Command response. |
| `3` | State change/snapshot. |
| `4` | Telemetry. |
| `5` | Heartbeat/management. |
| `6` | Commissioning/configuration. |
| `7` | Debug. |

## Current Executable Allocation

The current allocation is defined by `pdcan_protocol::MESSAGE_DEFINITIONS` and is
still subject to pre-v1 review. It includes:

- `EMERGENCY_DISABLE`;
- `SET_PORT_POLICY`;
- `ACKNOWLEDGE_EMERGENCY_RESOLVED`;
- `REQUEST_STATUS`;
- `COMMAND_RESPONSE`;
- `PORT_STATE`;
- `PORT_POWER`;
- `HEARTBEAT`;
- `BOARD_INFO`; and
- `NODE_CLAIM`.

Only the identifier layout and leading request ID are encoded in the initial
skeleton. Do not infer unimplemented payload fields from the original source plan.

## Compatibility and Update Boundary

Operational firmware and `pdcan` require an exact protocol version match for v1.
Discovery must remain tolerant enough to report UID, hardware revision, firmware
version, protocol version, and incompatibility without attempting operational
control.

CAN firmware update is out of scope for v1. SWD/service tooling is the update path.
A future bootloader must use a small stable management/update namespace
independent of the exact-match operational protocol so incompatible application
firmware can be reached. No updater, image slot, or update command is allocated.

## Emergency Disable

`EMERGENCY_DISABLE` is the highest-priority best-effort Rev A command. Its
backpack-wide latch persists across watchdog reset and complete power loss. A
successful response is sent only after the latched record is durable.

While latched, power-enabling/renegotiation commands are rejected. Clearing
requires the distinct `ACKNOWLEDGE_EMERGENCY_RESOLVED` command and means the
operator has declared normal operation safe. The cleared state is made durable
before success is reported, and clearing does not itself enable any port.

Rev A performs shutdown through the same I2C/SW3538 path as ordinary control.
It is not a safety-rated independent cutoff.

## Generated DBC

Run:

```text
cargo xtask dbc
cargo xtask dbc --check
```

The checked-in [`can/pdcan.dbc`](can/pdcan.dbc) is generated from protocol message
definitions. DBC identifiers use representative node, target, and requester values
because those fields are dynamic on the bus. CI fails if the generated file drifts.
