# Generation-2 Firmware Architecture

## Nodes and ownership

Every backplane and carrier is an independent STM32C092GCU6 CAN-FD node. The
host communicates with each node directly. A backplane never proxies carrier
commands, telemetry, commissioning, or firmware updates.

The backplane owns aggregate power monitoring, two fan channels, its 1-Wire
sensor bus, its status NeoPixel, and local safety. A carrier owns its protected
output, input monitoring, status NeoPixel, local safety, and any profile-specific
backend. The current carrier backend is SW3538.

Carrier binding is optional descriptive metadata containing a backplane UID and
zero-based slot. It does not create a control hierarchy and cannot be verified
electrically because every carrier sees the same CAN and power buses.

## Crate boundaries

| Path | Responsibility |
| --- | --- |
| `pdcan-types` | Hardware-neutral roles, profiles, capabilities, policy, telemetry, faults, and update state. |
| `pdcan-protocol` | Strict CAN-FD header/payload codecs and the canonical message catalog. |
| `pdcan-core` | Commissioning, safe carrier/backplane policy, emergency behavior, startup staggering, and update sequencing. |
| `pdcan-drivers` | Local INA237, DS18B20, and SW3538 access plus atomic flash records. |
| `pdcan-artifact` | Host-side update bundle parsing, construction, and SHA-256 validation. |
| `backplane-firmware` | Final backplane pin/peripheral contract and embedded application. |
| `carrier-firmware` | Final SW3538 carrier pin/peripheral contract and embedded application. |
| `pdcan-bootloader` | Embassy Boot swap, trial-boot, and rollback entry point. |

Protocol behavior branches on advertised capability bits. Profile identifiers
select hardware-specific implementations and compatibility, but clients must
not infer capabilities from a profile name.

## Final pin contracts

Both boards use FDCAN1 RX on PB0 and TX on PB1. TCAN3413 standby is tied low on
the PCB and has no MCU pin.

### Backplane

| Function | STM32 pin |
| --- | --- |
| NeoPixel data | PA1 |
| Fan 0 supply PWM / tach | PA2 / PB8 |
| Fan 1 supply PWM / tach | PA0 / PA8 |
| 12 V power good | PA3 |
| INA237 alert | PA4 |
| I2C2 SDA / SCL | PA6 / PA7 |
| DS18B20 1-Wire | PA15 |

### Current SW3538 carrier

| Function | STM32 pin |
| --- | --- |
| NeoPixel data | PB8 |
| PD enable | PB3 |
| PD power good | PA15 |
| PD interrupt | PC6 |
| Buck power good | PA8 |
| INA237 alert | PB5 |
| I2C1 SCL / SDA | PB6 / PB7 |
| User button (active low) | PC15 |

## Startup and safe states

Carrier `PD_ENABLE` is driven low before broader initialization. Persisted
enable policy is restored only after flash/config validation, the correct board
profile is confirmed, required local health inputs are good, and a deterministic
UID-derived delay in the 100–899 ms range expires. An emergency or safety fault
always forces the output off and dominates persisted policy.

Both backplane fan supply-PWM outputs start high (100% duty). Emergency handling
keeps cooling at that safe setting. Fan control is explicitly two-channel and
3-wire; there is no 4-wire control mode.

## Sensors

Both boards use a local INA237 at address `0x40`. The backplane's 1 mΩ aggregate
shunt uses the narrow ADC range and 1 mA current LSB. The carrier's 6 mΩ shunt
uses the wide range and 500 µA current LSB.

Temperature reports keep STM32 die, INA237 die, and each full 64-bit DS18B20 ROM
identity separate. Unavailable, stale, and invalid samples are explicit states;
one sensor is never substituted for another.

## Firmware updates and boot

The running application accepts `FW_BEGIN` and sequential `FW_DATA` frames into
the DFU partition, services safety/CAN/watchdog work between flash operations,
and verifies the complete SHA-256 digest at `FW_FINISH`. Finish records a staged
manifest but never marks the image updated or resets the MCU.

`FW_ACTIVATE` is separate. For an `interrupt` profile, it requires an explicit
allow-interruption bit, gracefully disables the service, calls Embassy Boot's
`mark_updated`, acknowledges, and resets. The current SW3538 carrier is
`interrupt`. A profile may report `live` only after HIL establishes continuity
through swap, reset, trial failure, and rollback—not merely during download.

On the first application boot, `mark_booted` is deferred until flash/config,
board identity, CAN, watchdog, and critical local peripheral checks pass. Host
presence is not required. Failure before confirmation leaves Embassy Boot able
to roll back. The bootloader can only be replaced or recovered over SWD.

## Flash layout

All boundaries are 2 KiB-page aligned:

| Partition | Address | Size |
| --- | ---: | ---: |
| Bootloader | `0x08000000` | 16 KiB |
| Bootloader state | `0x08004000` | 8 KiB |
| ACTIVE application | `0x08006000` | 110 KiB |
| DFU staging | `0x08021800` | 112 KiB |
| Reserved alignment page | `0x0803D800` | 2 KiB |
| Persistent configuration | `0x0803E000` | 8 KiB |

Application release images have a 110 KiB hard limit and 100 KiB growth budget.
The bootloader has a 16 KiB hard limit. `cargo xtask ci` checks these budgets.
