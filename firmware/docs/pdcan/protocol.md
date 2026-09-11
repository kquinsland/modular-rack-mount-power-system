# PDCAN Protocol 1.0

PDCAN is the direct host-to-node protocol for the Generation-2 backplane and
carrier boards. Every board is an independent STM32C092GCU6 CAN-FD node. A
backplane does not proxy commands, telemetry, commissioning, or firmware for a
carrier.

The executable wire contract is `pdcan-protocol`. The checked-in DBC at
`can/pdcan.dbc` is generated from that crate with `cargo xtask dbc`; generated
files must not be edited by hand.

## Link requirements

- CAN-FD frames with bit-rate switching are required.
- All PDCAN frames use 29-bit extended identifiers.
- Classic CAN, standard identifiers, and CAN-FD frames without BRS are ignored.
- Multi-byte integers are little-endian.
- Payload lengths use legal CAN-FD sizes. Reserved bytes must be zero.
- Protocol requester `0` is reserved for unsolicited node traffic. Host
  requesters are `1..=15`; `pdcan` defaults to `15`.
- Node `0` is the broadcast address and is currently valid only for emergency
  disable. Operational Node IDs are `1..=254`; `255` is invalid.

## Operational identifier

All classes except commissioning use this identifier layout:

| Bits | Width | Field |
| ---: | ---: | --- |
| 28..26 | 3 | Priority; lower wins arbitration |
| 25..22 | 4 | Message class |
| 21..14 | 8 | Node ID |
| 13..10 | 4 | Target (`15` means the whole node; fan targets are `0` or `1`) |
| 9..6 | 4 | Requester |
| 5..0 | 6 | Class-local opcode |

Classes are Control `0x0`, Response `0x1`, State `0x2`, Telemetry `0x3`,
Management `0x4`, Configuration `0x5` (reserved), Firmware `0x6`, Commissioning
`0xE`, and Debug `0xF` (reserved). Decoders enforce the priority, target,
requester, opcode, payload length, value range, and zero-reserved-byte rules for
each message.

Commissioning uses the same priority/class/requester/opcode positions but
replaces the node and target fields with a 12-bit collision-reducing token.
The token is an integrity/routing aid, not authentication.

## Message inventory

| Class | Priority | Message | Sender | Target | Bytes | Request ID |
| --- | ---: | --- | --- | --- | ---: | --- |
| Control | 0 | `EMERGENCY_DISABLE` | Host | Node/broadcast | 8 | yes |
| Control | 1 | `SET_CARRIER_POLICY` | Host | Node | 24 | yes |
| Control | 1 | `ACKNOWLEDGE_EMERGENCY` | Host | Node | 8 | yes |
| Control | 1 | `REQUEST_STATUS` | Host | Node | 8 | yes |
| Control | 1 | `SET_FAN` | Host | Fan | 12 | yes |
| Control | 1 | `SET_BINDING` | Host | Node | 24 | yes |
| Response | 2 | `COMMAND_RESPONSE` | Node | Node | 16 | yes |
| State | 3 | `NODE_STATE` | Node | Node | 16 | no |
| State | 3 | `FAN_STATE` | Node | Fan | 12 | no |
| Telemetry | 4 | `POWER` | Node | Node | 16 | no |
| Telemetry | 4 | `TEMPERATURE` | Node | Node | 16 | no |
| Management | 5 | `HEARTBEAT` | Node | Node | 16 | no |
| Management | 5 | `NODE_INFO` | Node | Node | 32 | no |
| Management | 5 | `BINDING` | Node | Node | 16 | no |
| Firmware | 5 | `FW_BEGIN` | Host | Node | 64 | yes |
| Firmware | 5 | `FW_DATA` | Host | Node | 64 | no |
| Firmware | 5 | `FW_FINISH` | Host | Node | 16 | yes |
| Firmware | 5 | `FW_ACTIVATE` | Host | Node | 16 | yes |
| Firmware | 5 | `FW_ABORT` | Host | Node | 16 | yes |
| Firmware | 5 | `FW_STATUS` | Host | Node | 8 | yes |
| Firmware | 5 | `FW_ACK` | Node | Node | 24 | yes/cumulative |
| Firmware | 5 | `FW_STATUS_RESPONSE` | Node | Node | 64 | yes |
| Commissioning | 6 | `NODE_CLAIM` | Node | Token | 16 | no |
| Commissioning | 6 | `DISCOVER` | Host | Token 0 | 8 | nonce |
| Commissioning | 6 | `DISCOVERY_RESPONSE` | Node | Token | 64 | nonce |
| Commissioning | 6 | `IDENTIFY` | Host | Token | 24 | yes |
| Commissioning | 6 | `ASSIGN_NODE` | Host | Token | 24 | yes |
| Commissioning | 6 | `CLEAR_NODE` | Host | Token | 20 | yes |
| Commissioning | 6 | `COMMISSIONING_RESULT` | Node | Token | 24 | yes |

The DBC defines byte offsets and scaling for these payloads. Rust codecs remain
authoritative where DBC cannot express invariants such as optional-field
zeroing, target/profile compatibility, or commissioning tokens.

## Discovery and capabilities

Discovery reports the STM32 96-bit UID, optional Node ID, commissioning state,
protocol version, application and bootloader versions, hardware revision,
partition layout, role, optional carrier profile, capability flags, and update
impact.

Roles are `backplane` and `carrier`. Carrier profiles currently include
`basic`, `sw3538`, `accessory`, and `high_power_240w`. A profile declares whether activation is
`live` or `interrupt`; clients must use the declaration returned by the node,
not infer it from a profile name. Optional `{backplane_uid, slot}` binding is
descriptive inventory metadata and is not an address or trust boundary.

## Commands, state, and telemetry

Mutating commands carry a 64-bit request ID. Nodes cache enough information to
make retransmission with the same requester/request ID idempotent. A response
echoes the request ID and opcode and returns a stable `CommandResult` plus a
message-specific detail value.

`EMERGENCY_DISABLE` is the only broadcast command and has highest protocol
priority. Each node independently enters its safe state and durably latches the
emergency condition. Acknowledgement clears the latch only after the physical or
policy cause is resolved; it does not automatically enable a carrier output.

Backplanes accept fan commands and report two fan channels. Carriers accept
carrier policy and optional binding commands. Commands that do not apply to a
node's advertised role/capabilities return `Unsupported` or `InvalidTarget`.

Power telemetry describes a node-local INA237 reading. Temperature telemetry
identifies its source and sensor: the backplane can report its external DS18B20
and MCU die temperature. Sequence counters and explicit valid/stale/unavailable
status let the host detect dropped or old samples.

## Firmware update

The host updates one node directly. The backplane never updates a carrier.

1. The host validates a versioned `.pdcan` bundle and sends `FW_BEGIN` with the
   target tuple, version, image length, and SHA-256 digest.
2. The node rejects role/profile, hardware revision, partition-layout, size, or
   update-state mismatches before erasing or writing staging flash.
3. The host sends 48-byte `FW_DATA` chunks in windows of up to eight frames.
   `FW_ACK.next_offset` is cumulative. Exact duplicates are accepted only when
   staged flash contains the same bytes; gaps and conflicting duplicates fail.
4. `FW_FINISH` is accepted only at the declared image length. The node hashes
   staged flash and enters `Staged` only when it matches the manifest.
5. `FW_ACTIVATE` marks the image for Embassy Boot and resets. If the node
   advertises `interrupt`, the request and operator command must explicitly
   allow interruption.
6. The application must call Embassy Boot's `mark_booted` inside a bounded
   health-confirmation window. Otherwise the bootloader reverts to the previous
   application.

`FW_STATUS` reports receiving/staged state, last error, running/staged version,
session, next offset, digest, and live/interrupt impact. `FW_ABORT` discards a
receiving or staged session. Host timeouts retransmit the current request or
data window; nodes must not rely on a single delivery.

The artifact format reserves an algorithm, key ID, and signature record. The
initial implementation verifies target, size, and SHA-256 integrity but accepts
unsigned artifacts; Ed25519 enforcement and provisioning policy must be added
before this is treated as an authenticated update channel.

## Compatibility rule

Major version mismatch is incompatible. A minor version may add messages or
capability-gated fields without changing existing encodings. Existing message
meaning, offsets, and enum values do not change within major version 1.
