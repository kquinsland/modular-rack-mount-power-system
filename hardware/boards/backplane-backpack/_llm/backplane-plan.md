# PDCAN Firmware and Tooling Implementation Plan

## 1. Purpose

This repository will implement firmware and host-side tooling for a CAN-FD-connected USB-C Power Delivery controller board built around an `STM32C092FCP6`.

Each board:

- exposes one CAN-FD node;
- controls up to eight identical hot-swappable USB-C PD modules;
- reaches those modules over one STM32 I²C peripheral through a `TCA9548APWR` 8-channel I²C mux;
- polls each installed PD module for connection state, negotiated contract information, electrical telemetry, temperature, and fault state;
- can send commands to each PD controller, including enabling/disabling the USB-C port, limiting advertised SPR capabilities, controlling EPR policy, requesting renegotiation, and clearing/resetting faults where supported;
- drives a NeoPixel-compatible status LED;
- drives an optional 3-pin or 4-pin fan interface;
- stores a commissioned CAN node address in internal flash;
- remains discoverable by the STM32's factory-programmed 96-bit unique ID before and after commissioning.

The repository also contains a Rust CLI, `pdcan`, for discovery, physical identification, commissioning, status inspection, and port control.

The implementation should prioritize deterministic behavior, easy host-side unit testing, robust hot-swap behavior, simple wire-level debugging, strict ownership of hardware peripherals, stable protocol boundaries, and clear separation between policy/business logic and hardware mechanisms.

---

## 2. Hardware assumptions

### 2.1 MCU

Target MCU:

```text
STM32C092FCP6
```

Relevant assumptions:

- the STM32C092 family includes FDCAN;
- the TSSOP20 package can use FDCAN in the application;
- the TSSOP20-specific limitation discussed during design is the ROM/system-memory bootloader's CAN boot support, not application access to the FDCAN peripheral;
- PA11/PA12 are expected to be used for FDCAN RX/TX if consistent with final PCB routing;
- firmware recovery/programming should continue to rely on SWD or another supported boot path rather than assuming CAN ROM bootloader availability.

The implementation team must verify all final pin assignments, timer selections, DMA requirements, and alternate-function conflicts against the final schematic.

### 2.2 I²C topology

```text
STM32 I2C
    |
    v
TCA9548APWR
    |
    +-- CH0 --> PD module 0
    +-- CH1 --> PD module 1
    +-- CH2 --> PD module 2
    +-- CH3 --> PD module 3
    +-- CH4 --> PD module 4
    +-- CH5 --> PD module 5
    +-- CH6 --> PD module 6
    `-- CH7 --> PD module 7
```

All eight modules are identical and may therefore use the same I²C address.

Only one TCA9548 channel should normally be enabled at a time. Preferred transaction pattern:

```text
select channel -> perform one logical module operation -> deselect all channels
```

This leaves absent, damaged, unpowered, or partially inserted modules isolated from the upstream bus except while actively probing or accessing that slot.

If the TCA9548 reset pin is connected to the STM32, firmware should use it as part of the I²C recovery strategy.

### 2.3 Hot-swappable PD modules

The removable I²C device and the USB-C cable attached to it are separate concepts. Firmware must distinguish:

```text
Is a PD module installed in this slot?
```

from:

```text
Does the installed PD module currently have a USB-C cable/partner attached?
```

A missing module, an installed module with no cable, an active PD contract, and a module fault must not collapse into one state.

Hot removal may occur during any I²C transaction. All I²C operations therefore require finite timeouts and explicit recovery behavior.

### 2.4 Fan

The board may support either:

- 3-pin fan behavior, where fan power is PWM-switched with a MOSFET; or
- 4-pin fan behavior, where the fan is continuously powered and its dedicated PWM input is driven.

The application-level API must present the same logical fan commands regardless of electrical implementation. If a tachometer input is routed, fan RPM and fan-fault detection should be supported without exposing timer details to business logic.

### 2.5 Status LED

The NeoPixel-compatible status LED is used for boot indication, normal/degraded/fault state, address-conflict indication, and manual physical identification during commissioning.

Identification mode should override ordinary informational status patterns. A critical fault may be given higher priority if desired.

---

## 3. Repository architecture

Use a Cargo workspace.

```text
pdcan/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── LICENSE
├── .gitignore
├── .cargo/
│   └── config.toml
│
├── crates/
│   ├── pdcan-types/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── node.rs
│   │       ├── port.rs
│   │       ├── policy.rs
│   │       ├── telemetry.rs
│   │       ├── health.rs
│   │       └── version.rs
│   │
│   ├── pdcan-core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── controller.rs
│   │   │   ├── event.rs
│   │   │   ├── action.rs
│   │   │   ├── slot.rs
│   │   │   ├── scheduler.rs
│   │   │   ├── commissioning.rs
│   │   │   ├── health.rs
│   │   │   └── fan.rs
│   │   └── tests/
│   │       ├── hotplug.rs
│   │       ├── policy.rs
│   │       ├── scheduling.rs
│   │       ├── commissioning.rs
│   │       └── recovery.rs
│   │
│   ├── pdcan-protocol/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── id.rs
│   │       ├── codec.rs
│   │       ├── commissioning.rs
│   │       ├── control.rs
│   │       ├── response.rs
│   │       ├── event.rs
│   │       ├── telemetry.rs
│   │       └── management.rs
│   │
│   └── pdcan-drivers/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs
│       │   ├── tca9548a.rs
│       │   ├── pd_controller.rs
│       │   ├── pd_bus.rs
│       │   ├── fan.rs
│       │   ├── neopixel.rs
│       │   └── config_store.rs
│       └── tests/
│           ├── tca9548a.rs
│           ├── pd_controller.rs
│           └── config_store.rs
│
├── firmware/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── memory.x
│   └── src/
│       ├── main.rs
│       ├── board.rs
│       ├── channels.rs
│       ├── action_executor.rs
│       ├── can.rs
│       └── tasks/
│           ├── mod.rs
│           ├── controller.rs
│           ├── can.rs
│           ├── fan.rs
│           ├── status.rs
│           └── supervisor.rs
│
├── pdcan/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── cli.rs
│       ├── can.rs
│       ├── discover.rs
│       ├── output.rs
│       └── commands/
│           ├── mod.rs
│           ├── scan.rs
│           ├── identify.rs
│           ├── assign.rs
│           ├── unassign.rs
│           ├── info.rs
│           ├── status.rs
│           └── port.rs
│
├── docs/
│   ├── protocol.md
│   ├── commissioning.md
│   ├── firmware-architecture.md
│   ├── can/
│   │   └── pdcan.dbc
│   └── adr/
│       ├── 0001-workspace-boundaries.md
│       ├── 0002-pure-event-action-core.md
│       ├── 0003-single-owner-i2c.md
│       └── ...
│
├── xtask/
│   ├── Cargo.toml
│   └── src/main.rs
│
└── scripts/
    └── can-vcan.sh
```

`xtask` is recommended because the workspace contains both host and embedded targets. Provide stable entry points such as:

```text
cargo xtask build
cargo xtask test
cargo xtask firmware
cargo xtask flash
cargo xtask vcan
cargo xtask dbc
```

---

## 4. Crate boundaries and dependency rules

### 4.1 Dependency graph

```text
                         +----------------+
                         |  pdcan-types   |
                         |    no_std      |
                         +-------+--------+
                                 |
                +----------------+----------------+
                |                |                |
                v                v                v
       +----------------+ +---------------+ +----------------+
       |   pdcan-core   | |pdcan-protocol | | pdcan-drivers  |
       | business logic | | CAN wire fmt  | | mechanisms     |
       | state machines | | codec / IDs   | | I2C / PD / fan |
       | policy/sched.  | |    no_std     | | LED / flash    |
       |    no_std      | +-------+-------+ +-------+--------+
       +-------+--------+         |                 |
               +------------------+-----------------+
                                  |
                                  v
                          +---------------+
                          |   firmware    |
                          | Embassy/STM32 |
                          +---------------+

                          +---------------+
                          |     pdcan     |
                          |   host CLI    |
                          +-------+-------+
                                  |
                           pdcan-protocol
```

### 4.2 `pdcan-types`

A `#![no_std]` vocabulary crate containing types such as `NodeId`, `PortId`, `NodeUid`, `TransactionId`, `PortPolicy`, `PortTelemetry`, `PortStatus`, `FaultFlags`, `FirmwareVersion`, and `HardwareRevision`.

It must not contain Embassy types, embedded-hal traits, SocketCAN types, CAN frame encoding, register addresses, or scheduling/state-machine behavior.

### 4.3 `pdcan-core`

A `#![no_std]` pure business-logic crate. It must not depend on Embassy, embedded-hal, STM32 HAL/PAC, CAN drivers, SocketCAN, flash drivers, wall-clock APIs, filesystems, or OS facilities.

The core receives events and produces actions:

```rust
pub enum Event {
    Tick { now_ms: u64 },
    Command(Command),
    ModuleProbeSucceeded { port: PortId, info: ModuleInfo },
    ModuleProbeFailed { port: PortId, error: DeviceError },
    StatusRead { port: PortId, status: PortStatus },
    StatusReadFailed { port: PortId, error: DeviceError },
    TelemetryRead { port: PortId, telemetry: PortTelemetry },
    TelemetryReadFailed { port: PortId, error: DeviceError },
    PolicyApplied { port: PortId },
    PolicyApplyFailed { port: PortId, error: DeviceError },
    ConfigPersisted,
    ConfigPersistFailed,
}

pub enum Action {
    ProbeModule { port: PortId },
    ReadStatus { port: PortId },
    ReadTelemetry { port: PortId },
    ReadTemperature { port: PortId },
    ApplyPolicy { port: PortId, policy: PortPolicy },
    ResetModule { port: PortId },
    Publish(OutboundMessage),
    PersistConfig(PersistentSettings),
    SetLed(LedState),
    SetFan(FanCommand),
}
```

The exact API may vary, but preserve:

```text
events in -> deterministic state transition -> actions out
```

### 4.4 `pdcan-protocol`

A `#![no_std]` shared wire-protocol crate responsible for 29-bit CAN ID construction/parsing, frame classes/types, payload layouts, explicit serialization/deserialization, validation, and compatibility tests.

Do not define the wire format by Rust ABI layout, `repr(C)` memory copies, serde, bincode, postcard, or another generic serializer. Encode/decode explicit bytes.

### 4.5 `pdcan-drivers`

Hardware mechanisms only:

- TCA9548 channel selection/deselection;
- PD-controller register transactions;
- raw-to-semantic conversions;
- applying semantic `PortPolicy` to registers;
- NeoPixel signaling;
- fan control;
- configuration flash mechanics.

Prefer generic `embedded-hal` / `embedded-hal-async` traits where practical so drivers can be host-tested with fakes.

Drivers answer **how** an operation is performed; `pdcan-core` decides **what** and **when**.

### 4.6 `firmware`

Connects STM32/Embassy peripherals to core, drivers, and protocol. It owns clocks, FDCAN, physical I²C, timers/PWM/DMA, watchdog, Embassy task/channel wiring, execution of `Action`, and conversion of hardware results back to `Event`.

Keep business logic out of this crate.

### 4.7 `pdcan`

Normal Rust host binary, initially targeting Linux SocketCAN. It handles discovery, physical identification, node assignment/removal, status, telemetry, control commands, timeouts, and human/machine-readable output while sharing `pdcan-protocol` with firmware.

---

## 5. Firmware concurrency model

Use a small number of long-lived Embassy tasks.

| Task | Exclusive ownership | Responsibility |
|---|---|---|
| `controller_task` | core controller state | Feed events to `pdcan-core`, schedule/dispatch actions |
| `can_task` | FDCAN peripheral | RX/TX and protocol encode/decode |
| PD/action executor | I²C + TCA9548 + PD devices | Execute PD-related core actions serially |
| `fan_task` | fan timer/GPIO/tach | Apply logical fan commands, collect tach if present |
| `status_task` | NeoPixel | Render semantic LED states/patterns |
| `supervisor_task` | watchdog policy | Health aggregation and watchdog feeding |

### 5.1 Single I²C owner

Do not create one task per PD module. All eight modules share one physical I²C controller through a mux and are inherently serialized.

Avoid eight tasks plus `Arc<Mutex<I2C>>`. Prefer one owner of I²C, TCA9548, and all eight downstream transactions.

This prevents mux races such as:

```text
task A selects channel 1
task B selects channel 5
task A reads and accidentally talks to channel 5
```

### 5.2 Core/action loop

```text
CAN command --------+
timer tick ---------+
hardware result ----+----> pdcan-core
                            |
                            v
                         Action
                            |
                            v
                      firmware executor
                            |
             +--------------+--------------+
             |              |              |
             v              v              v
           I2C/CAN         flash          LED/fan
             |
             v
        result Event
             |
             +-----------------------------> core
```

### 5.3 Explicit time

`pdcan-core` must not call an Embassy or host clock. Time is injected as data, e.g. `Event::Tick { now_ms }`.

This makes retries, timeouts, scheduling, and hot-swap behavior deterministic in unit tests.

---

## 6. PD slot model

A board contains eight stable logical slots. A slot remains meaningful even when its removable module is absent.

Suggested runtime shape:

```rust
pub struct Slot {
    pub id: PortId,
    pub module_state: ModuleState,
    pub usb_state: UsbState,
    pub desired: PortPolicy,
    pub telemetry: Option<PortTelemetry>,
    pub consecutive_errors: u8,
    pub last_seen_ms: Option<u64>,
}
```

### 6.1 Module lifecycle

```text
                 +---------+
                 | ABSENT  |
                 +----+----+
                      |
                      | valid PD controller appears
                      v
                +-----------+
                | PROBING   |
                +-----+-----+
                      |
                      | identity/status valid
                      v
               +--------------+
               | INITIALIZING |
               +------+-------+
                      |
                      | desired policy applied/verified
                      v
              +-----------------+
              |     ONLINE      |
              +--------+--------+
                       |
                       | repeated I2C failures
                       v
                +-------------+
                | RECOVERING  |
                +------+------+
                       |
                +------+------+
                |             |
                v             v
             ONLINE         ABSENT
```

A persistent positively identified failure may optionally transition to `FAULTED`.

### 6.2 USB-C state

Keep partner/contract state separate:

```rust
pub enum UsbState {
    Unknown,
    Disabled,
    NoCable,
    CableAttached,
    Negotiating,
    ContractActive,
    Faulted,
}
```

Examples:

```text
module=ONLINE, usb=NoCable
module=ONLINE, usb=ContractActive
module=ABSENT, usb=Unknown
```

---

## 7. Desired state versus observed state

Each slot maintains both.

Observed data includes module presence, enabled state, cable/partner status, contract validity, SPR/EPR state, CC orientation if available, VBUS voltage, current, power, temperature, negotiated contract voltage/current, and fault flags.

Example telemetry:

```rust
pub struct PortTelemetry {
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub power_mw: u32,
    pub temperature_cdeg: i16,
    pub contract_voltage_mv: u16,
    pub contract_current_ma: u16,
}
```

Desired policy example:

```rust
pub struct PortPolicy {
    pub enabled: bool,
    pub epr_allowed: bool,
    pub max_spr_voltage_mv: u16,
    pub max_spr_current_ma: u16,
    pub max_spr_power_mw: u32,
}
```

Desired policy belongs to the logical slot, not the removable module. A replacement module must receive slot policy before it is declared ready.

---

## 8. PD driver boundary

The PD-controller driver should expose semantic operations such as:

```text
probe
read_status
read_telemetry
read_temperature
set_enabled
set_spr_limit
set_epr_allowed
apply_policy
renegotiate
clear_fault
reset_controller
```

Only the driver should know register addresses, bit meanings, required write/read sequences, controller-specific delays, and scaling.

Never expose raw I²C register writes over CAN.

---

## 9. Hot-swap behavior

Hot-swap is normal operation, not an exceptional condition.

### 9.1 Presence probing

For an absent slot, select the slot, read a known identity/device-status register, validate it, and deselect. Prefer a real identity/known-value read over a bare ACK probe.

Initial cadence:

```text
ABSENT: probe every 250-500 ms
```

### 9.2 Removal

Do not declare removal after one failed transaction.

Initial policy:

```text
first failure:
    mark/retry transient error

multiple consecutive failures:
    RECOVERING

continued failures over bounded interval:
    ABSENT
```

Tune thresholds from real hardware behavior.

### 9.3 Finite timeouts

Every I²C transaction must have a finite timeout. One failed/wedged slot must never prevent service to the other seven slots or CAN.

### 9.4 Bus recovery

Recommended escalation:

```text
transaction failure
    -> deselect all TCA channels
    -> retry if appropriate
    -> reinitialize STM32 I2C if necessary
    -> GPIO/SCL bus recovery if required/supported
    -> reset TCA9548 if available
    -> resume probing
```

Maintain counters for recovery/error observability.

---

## 10. Polling and scheduling

Do not read every register at one rate.

| Data | Initial cadence |
|---|---:|
| module presence while absent | 250-500 ms |
| cable/CC/connection state | 50-100 ms |
| contract/status while connected | 100-250 ms |
| voltage/current/power | 100-250 ms while active |
| temperature | ~1 s |
| faults | 250 ms-1 s |
| persistently failed slot retry | 1-5 s |

Prefer contiguous/block reads where supported.

### 10.1 Commands outrank telemetry

Priority concept:

```text
highest:
    emergency disable
    disable port
    clear critical fault
    change power policy
    explicit status request

lower:
    state polling
    telemetry polling
    temperature polling
```

Control must not wait behind a complete telemetry sweep.

### 10.2 CAN congestion

Telemetry is freshness-oriented. Under TX congestion, continue PD management, retain/coalesce latest state, drop stale periodic telemetry if necessary, and preserve critical faults, commands, and responses. All queues must be statically bounded.

---

## 11. CAN-FD protocol

### 11.1 Goals

Use a custom protocol because the system has a small, well-defined device model and benefits from easy `candump` inspection.

Protocol properties:

- CAN-FD native;
- 29-bit extended IDs;
- semantic messages rather than register RPCs;
- idempotent control where practical;
- versionable;
- multiple boards per bus;
- independent of the PD controller's register map.

### 11.2 Operational CAN ID layout

Recommended baseline:

```text
 28    26 25       22 21              14 13         10 9              0
+--------+-----------+------------------+--------------+----------------+
|Priority|   Class   |     Node ID      |    Target    |      Type      |
| 3 bits |  4 bits   |      8 bits      |    4 bits    |    10 bits     |
+--------+-----------+------------------+--------------+----------------+
```

```rust
id =
    (priority as u32) << 26 |
    (class    as u32) << 22 |
    (node     as u32) << 14 |
    (target   as u32) << 10 |
    message_type as u32;
```

### 11.3 Node IDs

```text
0x00       broadcast
0x01-0xFE  commissioned nodes
0xFF       reserved / invalid
```

### 11.4 Targets

```text
0x0-0x7    slots/ports 0-7
0x8-0xD    reserved
0xE        all ports on selected node
0xF        board-level target
```

### 11.5 Classes

| Class | Purpose |
|---:|---|
| `0x0` | control |
| `0x1` | command response |
| `0x2` | state/event |
| `0x3` | telemetry |
| `0x4` | node management |
| `0x5` | persistent configuration |
| `0x6-0xD` | reserved |
| `0xE` | commissioning/discovery |
| `0xF` | debug/vendor development |

### 11.6 Priority

```text
0 emergency / critical fault
1 control command
2 command response
3 state-change event
4 telemetry
5 heartbeat/node management
6 commissioning/configuration
7 debug
```

### 11.7 Wire conventions

```text
byte order:      little-endian
voltage:         millivolts
current:         milliamps
power:           milliwatts
temperature:     signed centi-degrees Celsius
time:            explicitly ms or s per field
boolean:         0/1 or documented flags
reserved fields: transmit zero; receivers ignore
floating point:  never
```

---

## 12. Operational CAN messages

Recommended v1 set:

### Control

```text
SET_PORT_ENABLED
SET_PORT_POLICY
RENEGOTIATE
CLEAR_FAULT
REQUEST_STATUS
EMERGENCY_DISABLE
```

### Responses

Response type normally mirrors the control type. Include a `TransactionId` in request/response commands.

Suggested results:

```text
OK
OK_PENDING
INVALID_ARGUMENT
INVALID_TARGET
UNSUPPORTED
BUSY
NO_MODULE
NOT_READY
I2C_ERROR
PD_CONTROLLER_ERROR
TIMEOUT
VERIFY_FAILED
ADDRESS_CONFLICT
INTERNAL_ERROR
```

### State/events

```text
MODULE_INSERTED
MODULE_READY
MODULE_REMOVED
PORT_STATE
PORT_FAULT
```

### Telemetry

```text
PORT_POWER
```

### Management

```text
HEARTBEAT
BOARD_INFO
NODE_STARTED
```

---

## 13. Example payloads

These are starting layouts. Freeze exact v1 byte definitions in `docs/protocol.md` before implementation proceeds deeply.

### 13.1 Port telemetry, 16 bytes

```text
0-1     sample sequence        u16
2-3     voltage                u16 mV
4-5     current                u16 mA
6-9     power                  u32 mW
10-11   temperature            i16 centi-C
12-13   contract voltage       u16 mV
14-15   contract current       u16 mA
```

Measured electrical values remain separate from negotiated contract values.

### 13.2 Port state

Use a compact 16-byte frame containing sequence/version, state flags, fault flags, timestamp/uptime context, active PDO/profile if available, and useful PD state.

Suggested flags:

```text
ENABLED
CONNECTED
CONTRACT_VALID
EPR_ACTIVE
CC1_ACTIVE
CC2_ACTIVE
VBUS_PRESENT
FAULTED
```

### 13.3 Set port enabled

```text
0-1 transaction ID
2   enabled (0/1)
3-7 reserved
```

### 13.4 Set port policy, 16-byte starting layout

```text
0-1     transaction ID
2       flags:
            bit 0 enabled
            bit 1 EPR allowed
3       reserved
4-5     max SPR voltage mV
6-7     max SPR current mA
8-11    max SPR power mW
12-15   reserved
```

### 13.5 Heartbeat, 16 bytes

```text
0       protocol major
1       protocol minor
2       firmware major
3       firmware minor
4       firmware patch
5       hardware revision
6       health flags
7       reset reason
8-11    uptime seconds
12      online module bitmap
13      connected USB bitmap
14      enabled port bitmap
15      faulted port bitmap
```

Heartbeat cadence: approximately 1 Hz. Also retransmit periodic state snapshots so receivers can recover from missed events or late attachment.

---

## 14. Command semantics

Commands should describe desired state, not toggles or register operations.

Prefer `SET_PORT_ENABLED(false)` over `TOGGLE_PORT`, and `SET_PORT_POLICY(...)` over PDO/register-specific commands.

### 14.1 Idempotency

Retransmitting desired-state commands should be harmless. Use transaction IDs for request/response correlation and duplicate suppression where necessary. A small recent-transaction cache is sufficient.

### 14.2 Completion versus renegotiation

`OK` means the board/controller accepted/applied/verified the requested operation. It does not necessarily mean USB-PD renegotiation has completed.

```text
SET_PORT_POLICY
    -> COMMAND_RESPONSE: OK
    -> PD negotiation
    -> PORT_STATE: CONTRACT_CHANGED
    -> new PORT_TELEMETRY
```

### 14.3 Commands while module is absent

Desired-state commands such as `SET_PORT_ENABLED` and `SET_PORT_POLICY` may update slot policy and return `OK_PENDING`; the policy is applied when a valid module appears.

Hardware-action commands such as `RENEGOTIATE`, `CLEAR_FAULT`, or reset return `NO_MODULE`/`NOT_READY` when the module is absent.

---

## 15. Manual commissioning model

Commissioning is intentionally manual and physically verifiable.

Each board has:

```text
permanent identity:
    STM32 factory 96-bit UID

installation-specific address:
    8-bit CAN Node ID stored in flash
```

Expected workflow:

```text
pdcan scan
    -> list UUIDs, node IDs, and state

pdcan identify <uuid>
    -> selected physical board blinks

user observes physical location

pdcan assign <uuid> 3
    -> Node ID 3 persisted

pdcan scan
    -> board now appears as node 3
```

Do not infer physical chain order automatically.

### 15.1 Node states

```rust
pub enum NodeState {
    Uncommissioned,
    Active,
    AddressConflict,
}
```

### 15.2 Flash validity

Persistent records contain a magic value, format version, payload, and CRC. Invalid/absent records mean `Uncommissioned`; do not rely solely on erased `0xFF`.

### 15.3 UUID-addressed provisioning

Commissioning operations use the full UID and remain available after assignment:

```text
DISCOVER
DISCOVERY_RESPONSE
GET_INFO
IDENTIFY
ASSIGN_NODE
CLEAR_NODE
NODE_CLAIM
RESULT
```

### 15.4 Discovery arbitration

Multiple boards may respond simultaneously, so they must not use one common response CAN ID with different payloads.

Recommended approach:

1. host sends `DISCOVER` with a nonce;
2. each board computes `CRC32(UID || nonce)`;
3. a truncated token is incorporated into the commissioning arbitration ID;
4. the full 96-bit UID remains authoritative in the CAN-FD payload;
5. the CLI may perform multiple rounds with different nonces and merge results.

The exact commissioning ID layout must be frozen in `docs/protocol.md`.

### 15.5 Identify

Suggested semantics:

```text
IDENTIFY {
    uuid,
    duration_seconds
}
```

Default roughly 30 seconds. `duration_seconds = 0` may stop identification. Identification must not alter PD behavior.

### 15.6 Assign node

```text
ASSIGN_NODE(uid, node_id)
    -> validate
    -> persist
    -> read/CRC verify
    -> transition ACTIVE
    -> acknowledge/announce
```

A reboot should not be required if a clean in-place transition is practical.

### 15.7 Host address-availability check

Before assignment, `pdcan` checks whether the requested Node ID appears occupied and refuses by default if it is.

### 15.8 Duplicate-address defense

Two boards may be commissioned separately with the same address and later connected.

At startup, commissioned boards should use a UID-derived `NODE_CLAIM` phase. If distinct UIDs claim one Node ID, affected boards enter `AddressConflict`, suppress ordinary Node-ID traffic, and remain accessible through UUID commissioning operations.

---

## 16. Persistent configuration

At minimum, persist the Node ID.

Future-safe logical shape:

```rust
pub struct PersistentConfig {
    pub format_version: u16,
    pub commissioning: CommissioningConfig,
    pub port_defaults: [PortPolicy; 8],
    pub fan: FanConfig,
}
```

Whether per-port policy is persistent in v1 remains a product decision. Normal transient control must not implicitly write flash.

The storage layer must use a dedicated flash region, include format version/integrity checking, tolerate interrupted writes, expose semantic `load()`/`save()` APIs, and hide raw flash addresses from application logic.

---

## 17. Fan architecture

Expose one logical API:

```rust
pub enum FanCommand {
    Off,
    FullSpeed,
    Duty(u8),
}
```

Potential status:

```rust
pub struct FanStatus {
    pub commanded_duty: u8,
    pub rpm: Option<u16>,
    pub fault: bool,
}
```

The application and CAN protocol should not care whether the populated hardware is 3-pin power PWM or 4-pin control PWM.

---

## 18. Status LED architecture

Application code expresses semantic states, for example:

```rust
pub enum SystemStatus {
    Booting,
    Operational,
    Degraded,
    CanOffline,
    SensorBusFault,
    AddressConflict,
    Fatal,
}
```

`status_task` maps states to colors/patterns. `IDENTIFY` is a temporary overlay. NeoPixel timing remains outside business logic.

---

## 19. Supervisor and watchdog

Do not feed the watchdog from an unconditional timer.

Critical tasks report progress to a supervisor. Feed the hardware watchdog only if required subsystems are making acceptable progress.

Track at least controller-loop progress, PD/I²C executor progress, CAN task liveness/bus health, online-module count, I²C error/recovery counts, configuration integrity, and fan state if safety-relevant.

---

## 20. Host CLI

Recommended command surface:

```text
pdcan
├── scan
├── identify <uuid>
├── assign <uuid> <node>
├── unassign <uuid>
├── info <node-or-uuid>
├── status [node]
└── port
    ├── status <node.port>
    ├── enable <node.port>
    ├── disable <node.port>
    ├── policy <node.port> ...
    ├── renegotiate <node.port>
    └── clear-fault <node.port>
```

Examples:

```text
pdcan scan
pdcan identify 0037002D3432511420393837
pdcan assign 0037002D3432511420393837 3
pdcan status 3
pdcan port status 3.4
pdcan port disable 3.4
```

Use `node.port` as the preferred human shorthand. Support human-readable output and structured `--json` output.

---

## 21. Testing strategy

### 21.1 `pdcan-core`

This should carry most behavioral coverage and need no Embassy runtime or hardware mocks.

Required hot-swap tests:

- absent slot is periodically probed;
- valid insertion progresses through probing/initialization;
- desired policy is applied before `MODULE_READY`;
- removal during normal polling;
- removal during a pending operation;
- transient I²C errors do not immediately mean removal;
- repeated failure transitions through recovery to absent;
- replacement module inherits slot policy.

Required command tests:

- enable/disable idempotency;
- policy changes online and absent;
- absent desired-state commands become pending;
- hardware-action commands on absent modules return `NO_MODULE`;
- emergency disable outranks telemetry;
- duplicate transaction IDs do not repeat non-idempotent effects.

Required scheduling/commissioning/fault tests:

- active versus absent polling cadence;
- explicit simulated time;
- invalid flash -> uncommissioned;
- assign produces persistence action;
- identify state;
- duplicate node claim -> conflict;
- I²C timeout/recovery escalation;
- watchdog/health degradation.

### 21.2 `pdcan-protocol`

Use exact golden vectors: semantic message -> exact 29-bit CAN ID + exact payload bytes, and reverse decoding.

Cover invalid DLC, invalid enums, reserved fields, endianness, boundary IDs, malformed commissioning frames, and round trips.

Once v1 is released, these tests are the compatibility contract.

### 21.3 `pdcan-drivers`

Use fake `embedded-hal-async` implementations to verify exact TCA and PD-controller operations, including channel selection, deselection on success/failure, register sequences, scaling, timeouts, invalid identity, and recovery.

### 21.4 CLI integration

Use Linux `vcan` plus a simulated peer. Test scan aggregation, identify, assign/unassign, occupancy checks, request/response timeouts, retries, JSON stability, and `node.port` parsing.

A small simulator executable/crate is recommended if it keeps these tests simple.

### 21.5 Firmware CI

CI should build/test host crates and build the STM32 release target.

### 21.6 Hardware-in-the-loop

Before release validate all eight slots populated and partially populated; rapid insertion/removal; removal during active I²C transactions; abnormal SDA/SCL behavior; TCA reset recovery; multiple CAN nodes; duplicate IDs; CAN bus-off/recovery; fan modes; NeoPixel identify; and power-cycle persistence.

---

## 22. Protocol documentation and DBC

Maintain `docs/protocol.md` and `docs/can/pdcan.dbc`.

`protocol.md` is authoritative for ID allocation, classes/types, payloads, units, result codes, commissioning, broadcasts, and versioning.

The DBC should describe ordinary operational frames where practical. Commissioning may require additional prose because UID/nonce-derived arbitration IDs do not fit ordinary fixed-ID DBC conventions cleanly.

Do not silently alter released payloads. Introduce a new type/version for incompatible changes.

---

## 23. Broadcast policy

Global broadcast control should be tightly restricted.

Reasonable broadcasts:

```text
DISCOVER
EMERGENCY_DISABLE
```

Normal `ENABLE`, `SET_PORT_POLICY`, persistent config, `ASSIGN_NODE`, and `CLEAR_NODE` should require explicit node/port or UUID targets.

---

## 24. Error and health telemetry

Maintain counters for per-slot I²C failures/timeouts/recoveries, insert/remove count, policy failures, TCA resets, global I²C resets, CAN RX/TX errors, CAN bus-off events, config validation failures, and watchdog/reset reason.

Not every counter belongs in the heartbeat; expose detailed counters through board info/diagnostic queries as appropriate.

---

## 25. Implementation phases

### Phase 0: Freeze hardware interfaces

Close the final MCU pin map, PD-controller register documentation, TCA address/reset wiring, fan pins/mode, NeoPixel implementation, flash region, CAN transceiver details, and CAN nominal/data bitrate/BRS settings.

### Phase 1: Workspace and shared types

Create the Cargo workspace, `pdcan-types`, CI, formatting/linting, and embedded target build plumbing.

### Phase 2: Protocol v1 skeleton

Implement operational ID codec, commissioning-ID scheme, frame types, golden vectors, initial protocol docs, and DBC. Review/freeze frame layouts before deep CLI/firmware integration.

### Phase 3: Pure core

Implement `Controller`, `Event`, `Action`, slot state machine, explicit-time scheduler, desired/observed policy, commissioning state, identify behavior, address-conflict behavior, and unit tests.

### Phase 4: Drivers

Implement TCA9548, PD-controller probe/status/telemetry/policy, hot-swap errors, config store, fan, NeoPixel, and fake-bus tests.

### Phase 5: STM32/Embassy firmware bring-up

Implement board initialization, executor/tasks, FDCAN, I²C/TCA, action execution, supervisor/watchdog, and embedded logging. Prove one slot end-to-end before scaling to eight.

### Phase 6: `pdcan` commissioning CLI

Implement SocketCAN, scan, identify, assign, unassign, info, JSON output, and `vcan` integration tests.

### Phase 7: Eight-slot hot-swap management

Implement all slots, dynamic insertion/removal, scheduling, recovery, policy application, lifecycle events, and diagnostics.

### Phase 8: Port control and telemetry CLI

Implement status, enable/disable, policy, renegotiate, clear-fault, and decoded telemetry.

### Phase 9: Fan/status/system hardening

Complete fan modes, tach if present, LED patterns, identify overlay, watchdog criteria, CAN congestion behavior, bus-off recovery, and fault injection.

### Phase 10: HIL acceptance and protocol freeze

Execute the full hardware matrix and freeze protocol v1.

---

## 26. Definition of done

A v1 implementation is complete when:

1. A fresh board boots uncommissioned and is discoverable by UID.
2. `pdcan scan` lists all boards with UUID, Node ID, and commissioning state.
3. `pdcan identify <uuid>` visibly identifies exactly the intended board.
4. `pdcan assign <uuid> N` persists across power cycles.
5. Duplicate Node IDs are detected and recoverable through UUID commissioning.
6. Up to eight modules can be inserted/removed independently without wedging I²C or firmware.
7. A removed module does not block remaining slots.
8. Replacement modules receive slot policy before being declared ready.
9. Port state distinguishes module presence from cable/contract state.
10. Voltage/current/power/temperature and contract telemetry are available over CAN-FD.
11. Hosts can semantically enable/disable ports and apply PD policy.
12. High-priority control is not blocked by routine telemetry.
13. Heartbeat/state snapshots allow late listeners to reconstruct state.
14. Protocol has exact golden-vector tests.
15. Business logic is comprehensively testable with normal host `cargo test`.
16. PD/TCA drivers have fake-I²C tests.
17. CLI behavior has `vcan` integration tests.
18. Embedded firmware builds reproducibly in CI.
19. HIL covers hot removal mid-transaction, recovery, multiple CAN nodes, and address conflicts.
20. Protocol docs, DBC, and ADRs match released implementation.

---

# 27. Architecture Decision Records (ADRs)

The team should keep decisions in `docs/adr/`.

Use a lightweight format:

```markdown
# ADR-NNNN: Title

- Status: Proposed | Accepted | Superseded
- Date: YYYY-MM-DD

## Context

What problem or constraint caused this decision?

## Decision

What are we doing?

## Consequences

What becomes easier, harder, or constrained?

## Alternatives considered

What other reasonable options were rejected and why?
```

The following ADRs should exist at project start.

---

## ADR-0001: Use a Cargo workspace with explicit crate boundaries

**Status:** Accepted

### Context

The project contains embedded firmware, hardware drivers, substantial business logic, a shared CAN protocol, and a host CLI. Testing business logic directly against Embassy and STM32 peripherals would create unnecessary coupling and complicated mocks.

### Decision

Use separate crates for:

```text
pdcan-types
pdcan-core
pdcan-protocol
pdcan-drivers
firmware
pdcan
```

### Consequences

Benefits are clear dependency direction, normal host-side unit testing, protocol reuse by firmware and CLI, and future reuse by another host tool or MCU. The cost is additional Cargo manifests and deliberate placement of shared types.

---

## ADR-0002: Business logic is a pure event/action state machine

**Status:** Accepted

### Context

The difficult behavior is hot-swap lifecycle, retries, scheduling, desired versus observed state, commissioning, conflicts, faults, and command priority. These need fast deterministic tests.

### Decision

`pdcan-core` receives events and emits actions. It has no Embassy runtime, hardware access, wall clock, or I/O.

### Consequences

This enables deterministic tests, simulated time, easy fault injection, and possible reuse in a simulator. Firmware must provide a small action-execution adapter.

---

## ADR-0003: One task owns I²C, TCA9548, and all eight PD modules

**Status:** Accepted

### Context

All modules share one physical I²C controller through a mux. Per-port tasks do not add I²C parallelism and can create mux-selection races.

### Decision

One execution path serializes:

```text
select mux channel
perform complete logical operation
deselect
```

Do not share the I²C peripheral among eight tasks through `Arc<Mutex<_>>`.

### Consequences

Mux-selection races are removed and recovery is simpler. Long operations on one module can delay others, so finite timeouts and command priority are mandatory.

---

## ADR-0004: Treat removable PD assemblies as stable logical slots

**Status:** Accepted

### Context

Modules are hot-swappable, while the physical position and policy assigned to that position remain meaningful.

### Decision

Model eight stable slots. Module lifecycle is dynamic within a slot, and desired policy belongs to the slot rather than to the currently installed module.

### Consequences

Replacement modules inherit the position's policy and can be initialized safely before being declared ready.

---

## ADR-0005: Separate module presence from USB-C partner state

**Status:** Accepted

### Context

An absent PD module and an installed module with no USB-C cable are operationally different.

### Decision

Maintain separate `ModuleState` and `UsbState` and expose that distinction over CAN and in the CLI.

### Consequences

Diagnostics and automation can distinguish hardware presence, cable presence, negotiation, and faults correctly.

---

## ADR-0006: Hot-swap failures are normal lifecycle events

**Status:** Accepted

### Context

A user may remove a PD module during an I²C transaction.

### Decision

All I²C operations use finite timeouts. Repeated errors drive explicit recovery/absence transitions rather than wedging or crashing the system.

### Consequences

Healthy slots continue operating when another module is removed or malfunctioning.

---

## ADR-0007: Drivers expose semantic PD operations, not registers

**Status:** Accepted

### Context

The PD controller may change between board revisions, and its register map should not leak into business logic or CAN.

### Decision

Expose operations such as `read_telemetry`, `set_enabled`, `apply_policy`, `renegotiate`, and `clear_fault`. Do not expose raw register writes in the public protocol.

### Consequences

A future controller replacement is mostly confined to the driver crate.

---

## ADR-0008: Use a custom CAN-FD protocol with 29-bit extended IDs

**Status:** Accepted as design baseline; exact allocations must be frozen before v1.

### Context

The device model is small and specific, and requires easy debugging, node/port addressing, priority arbitration, commissioning, and compact telemetry.

### Decision

Use a custom CAN-FD protocol with an operational ID structure based on:

```text
priority | class | node | target | type
```

### Consequences

The protocol maps cleanly to the product and is easy to inspect, but the project owns compatibility, documentation, and tooling instead of inheriting CANopen/J1939 conventions.

---

## ADR-0009: Explicit byte-level serialization

**Status:** Accepted

### Context

Generic serializers and Rust struct memory layout can change independently of the desired long-lived wire format.

### Decision

Encode/decode payloads explicitly with documented widths, units, and endianness.

### Consequences

There is more boilerplate, but wire compatibility is stable and golden-vector testing is straightforward.

---

## ADR-0010: Commands express desired state and are idempotent where possible

**Status:** Accepted

### Context

CAN frames/responses can be lost or retried.

### Decision

Use semantic desired-state operations such as `SET_PORT_ENABLED(false)` and `SET_PORT_POLICY(...)` rather than toggles. Use transaction IDs for correlation and duplicate suppression where needed.

### Consequences

Retries are predictable and safer.

---

## ADR-0011: Control commands outrank periodic telemetry

**Status:** Accepted

### Context

A request to disable a USB-C port should not wait behind a full eight-port telemetry sweep.

### Decision

The core scheduler prioritizes control and fault actions over routine telemetry. Telemetry may be coalesced/dropped under congestion.

### Consequences

Telemetry is explicitly freshness-oriented rather than guaranteed-delivery-oriented.

---

## ADR-0012: Manual UUID-based commissioning with flash-persisted Node IDs

**Status:** Accepted

### Context

Multiple identical boards may exist in one system, and physical position is most reliably assigned by a human during installation.

### Decision

Use the STM32 96-bit UID as permanent identity. The user scans for UUIDs, asks a UUID to blink, physically identifies the board, chooses a Node ID, and persists that ID in flash.

### Consequences

No DIP switches or address GPIOs are needed, while every board remains permanently identifiable.

---

## ADR-0013: Commissioning remains available by UID after node assignment

**Status:** Accepted

### Context

Users must be able to answer "which physical board is Node N?" and recover from address conflicts.

### Decision

`DISCOVER`, `IDENTIFY`, `ASSIGN_NODE`, and `CLEAR_NODE` remain UUID-addressable after commissioning.

### Consequences

Physical identification and address repair do not require reflashing or jumpers.

---

## ADR-0014: Discovery responses use UID/nonce-derived arbitration

**Status:** Accepted in principle; exact ID allocation remains to be frozen.

### Context

Multiple boards cannot safely send the same CAN ID at the same instant with different discovery payloads.

### Decision

Discovery includes a nonce. Each board derives an arbitration token from `UID || nonce`; the full UID remains in the payload.

### Consequences

CAN arbitration serializes simultaneous discovery responses. Multiple rounds resolve rare truncated-token collisions.

---

## ADR-0015: Defend against duplicate commissioned Node IDs

**Status:** Accepted

### Context

Two boards may be commissioned separately with the same Node ID and later connected to one bus.

### Decision

Use a UID-derived `NODE_CLAIM` startup phase. If multiple UIDs claim one Node ID, affected boards enter `AddressConflict`, suppress normal Node-ID traffic, and remain reachable through UUID commissioning.

### Consequences

Address conflicts are detectable and recoverable without reflashing.

---

## ADR-0016: Use periodic snapshots in addition to state-change events

**Status:** Accepted

### Context

CAN frames may be lost and monitoring software may attach after the system has already started.

### Decision

Publish immediate state-change events, an approximately 1 Hz node heartbeat, and periodic current-state snapshots.

### Consequences

Receivers can reconstruct current state without assuming perfect delivery.

---

## ADR-0017: Keep measured telemetry separate from negotiated contract values

**Status:** Accepted

### Context

A port may negotiate 20 V / 5 A but currently consume 20 V / 2 A.

### Decision

Represent actual voltage/current/power and negotiated contract limits independently.

### Consequences

Host tooling can distinguish available capacity from real consumption.

---

## ADR-0018: NeoPixel accepts semantic state and supports an identification overlay

**Status:** Accepted

### Context

Business logic should not contain RGB/timing details, while commissioning requires obvious physical identification.

### Decision

The status task maps semantic state to LED patterns. `IDENTIFY` temporarily overlays a distinctive blink pattern.

### Consequences

LED appearance can evolve independently of commissioning and business logic.

---

## ADR-0019: Fan hardware is abstracted behind one logical interface

**Status:** Accepted

### Context

The PCB can support either 3-pin power-PWM or 4-pin control-PWM fan operation.

### Decision

Expose logical duty/off/full-speed behavior and hide the electrical implementation below it.

### Consequences

Business logic and CAN do not care which fan option is populated.

---

## ADR-0020: Feed the watchdog only when critical subsystems show progress

**Status:** Accepted

### Context

An unconditional watchdog-feed task can keep a partially wedged firmware alive indefinitely.

### Decision

A supervisor aggregates subsystem liveness and feeds the hardware watchdog only when required tasks are making progress.

### Consequences

The watchdog becomes a meaningful recovery mechanism.

---

## ADR-0021: Maintain a shared protocol crate and DBC

**Status:** Accepted

### Context

The embedded device and Rust host CLI must not drift in their interpretation of CAN frames.

### Decision

Both consume `pdcan-protocol`. Maintain `docs/protocol.md`, a DBC for normal frames, and exact golden-vector tests.

### Consequences

There is one source of truth for the wire interface.

---

## ADR-0022: Persistent port-policy behavior is deferred

**Status:** Proposed / pending product decision

### Context

The Node ID must persist. It is not yet necessary to decide whether ordinary per-port runtime policy changes survive reboot.

### Decision

Design the configuration format to support persistent per-port defaults, but initially persist only explicitly required settings. Ordinary control commands must not implicitly write flash.

### Consequences

Flash wear and policy semantics remain predictable, while persistent defaults can be added without redesigning storage.

---

## 28. Open items the implementation team must resolve

1. Exact USB-C PD controller part number and complete register map.
2. Exact controller semantics required to disable CC1/CC2, limit SPR, permit/deny EPR, request renegotiation, read the negotiated contract, read V/I/P/temperature, clear faults, and reset the controller.
3. Final STM32 pin map and alternate-function verification.
4. TCA9548 reset-pin availability.
5. Electrical I²C hot-swap behavior, including downstream pull-ups, behavior while modules are unpowered, SDA/SCL isolation, and protection components.
6. CAN nominal/data bitrate and whether BRS is enabled.
7. Exact v1 CAN class/type values and commissioning-ID bit allocation.
8. Whether per-port default policy is persisted in v1.
9. Exact fan pin/tach capability and whether 3-pin/4-pin behavior is compile-time selected.
10. Exact LED status-pattern definitions.
11. Watchdog timeout and subsystem liveness criteria.
12. CI/HIL environment and flashing/debug tooling.

---

## 29. Guiding implementation rule

When deciding where code belongs, use:

```text
pdcan-core:
    WHAT should happen, and WHEN?

pdcan-drivers:
    HOW is the hardware operation performed?

pdcan-protocol:
    HOW is semantic state represented on CAN?

firmware:
    CONNECT the core, drivers, Embassy, and STM32 peripherals.

pdcan:
    PRESENT and CONTROL the system for a human/operator.
```

If code cannot be placed cleanly using those questions, treat that as an architectural smell and review the boundary before continuing.
