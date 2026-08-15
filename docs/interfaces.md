# Interfaces

This document describes the active Backplane Backpack architecture. All firmware,
CLI, JSON, diagnostic, and board port identifiers are zero-based.

## Backplane DC Bus

Backplane `J14` (`DC_IN`) and `J15` (`DC_OUT`) use DEGSON
`DG135T-10.16-02P-14-00A(H)`, LCSC `C708738`. `J15` repeats the bus for an optional
downstream backplane.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Shared | Common DC return. |
| 2 | `+24V` | Input/output | Main nominal 24 V bus. |

Each electrical pin maps to the terminal's two internally common through-hole
pins. The connector is not polarized, so silkscreen must clearly identify both
nets.

## Backpack Power Input

Backplane Backpack Rev A `J1` supplies the controller, fan regulator, and local
logic. Its mechanical series is not yet locked.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Input | Common return. |
| 2 | `VIN_RAW` | Input | Approximately 20–36 V; nominal system input is 24 V. |

## Backpack CAN-FD

Backplane Backpack Rev A `J2` is the protected CAN-FD connection. The connector's
mechanical series is not yet locked.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Shared | CAN reference/return. |
| 2 | `CANL` | Bidirectional | Bus-side signal after choke and ESD protection. |
| 3 | `CANH` | Bidirectional | Bus-side signal after choke and ESD protection. |

`SJ1` enables the local 120 ohm termination only when the backpack is at a physical
bus endpoint. The v1 working network configuration is 1 Mbit/s nominal arbitration,
2 Mbit/s data phase, and CAN-FD bit-rate switching enabled. The physical CAN segment
is trusted; v1 provides no authentication or encryption.

## Backpack Downstream I2C Ports

Rev A connectors `J3` through `J8` map to logical ports and TCA9548A channels 0
through 5. Their mechanical series is not yet locked.

| Connector | Logical port | Mux channel | Pin 1 | Pin 2 | Pin 3 |
| --- | ---: | ---: | --- | --- | --- |
| `J3` | 0 | 0 | `GND` | `I2C0_SCL` | `I2C0_SDA` |
| `J4` | 1 | 1 | `GND` | `I2C1_SCL` | `I2C1_SDA` |
| `J5` | 2 | 2 | `GND` | `I2C2_SCL` | `I2C2_SDA` |
| `J6` | 3 | 3 | `GND` | `I2C3_SCL` | `I2C3_SDA` |
| `J7` | 4 | 4 | `GND` | `I2C4_SCL` | `I2C4_SDA` |
| `J8` | 5 | 5 | `GND` | `I2C5_SCL` | `I2C5_SDA` |

Mux channels 6 and 7 terminate at test pads on Rev A and are not supported ports.
Firmware must never probe them in the Rev A build. Downstream pull-ups are DNP by
default when the attached SW3538 module supplies appropriate pull-ups.

## Backpack Fan

Rev A `J9` accepts the standard four-position PC-fan pinout and can be configured
electrically for a 3-wire or 4-wire fan with `SJ2` and `SJ3`.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Backpack to fan | Fan return. |
| 2 | `FAN_V+` | Backpack to fan | Switched or direct 12 V according to `SJ2`. |
| 3 | `FAN_TACH` | Fan to backpack | 3.3 V pull-up; PA1/TIM17 capture. |
| 4 | `FAN_PWM` | Backpack to fan | Open-drain PWM; unused for 3-wire. |

Firmware defaults to 3-wire mode and full speed. `pdcan` can persist the
selected electrical mode and requested duty. Boot and safety/fault behavior
always start or override to the mode-appropriate full-speed state.

## Backpack SWD

Rev A `J10` provides the programming/debug signals with the following schematic
pinout:

| Logical pin | Signal |
| ---: | --- |
| 1 | `GND` |
| 2 | `+3V3` |
| 3 | `SWDIO` |
| 4 | `SWCLK` |
| 5 | `NRST` |

SWD/service tooling is the v1 firmware-update path; CAN-based update is out of
scope for v1.

## Backplane to Carrier Slot

The high-current backplane/carrier interface remains separate from the backpack's
I2C connectors.

| Signal | Direction | Notes |
| --- | --- | --- |
| `VDC_SLOT` | Backplane to carrier | Nominal 24 V module input. |
| `GND` | Shared | Power and signal return. |
| `SDA_SLOT_N` | Bidirectional | Slot-local SW3538 I2C data. |
| `SCL_SLOT_N` | Backpack to carrier | Slot-local SW3538 I2C clock. |

Carrier SW3538 modules use the same I2C address; the backpack mux provides one
isolated segment per supported port.

## Logical Capacity and Board Revisions

PDCAN, shared crates, persistent records, and simulation support ports 0
through 7. Each firmware image embeds its hardware revision and supported-port
bitmap:

| Board | Supported bitmap | Ports |
| --- | ---: | --- |
| Backplane Backpack Rev A | `0x3F` | 0–5 |
| Future eight-port revision | `0xFF` | 0–7 |

Commands targeting an unsupported port return `UNSUPPORTED`/`INVALID_TARGET`; they
must not be reported as an absent module.
