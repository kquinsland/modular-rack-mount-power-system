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

## Backpack-to-Backplane Control Boundary

The stable Rev B backplane prototype owns the TCA9548A mux, PCA9554 power
expander, per-slot FET circuits, slot-local pull-ups/ESD, and every carrier
connector. Its `J2` (`i2c_backpack`) boundary exposes only the upstream I2C bus
and logic power:

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Shared | Logic return, duplicated with pin 3. |
| 2 | `+3V3` | Backpack to backplane | Backplane control-logic supply, duplicated with pin 4. |
| 3 | `GND` | Shared | Logic return, duplicated with pin 1. |
| 4 | `+3V3` | Backpack to backplane | Backplane control-logic supply, duplicated with pin 2. |
| 5 | `I2C_UP_SDA` | Bidirectional | Upstream data shared by PCA9554 and TCA9548A. |
| 6 | `I2C_UP_SCL` | Backpack to backplane | Upstream clock from STM32 I2C2. |

No mux-reset, PCA9554 interrupt, per-slot I2C, or per-slot power-control signal
crosses this connector. The backplane-local TCA9548A reset is held inactive by a
4.7 kohm pull-up and is not firmware-controlled. The five former shift-register
GPIOs and legacy PA3 mux-reset signal are unused by Rev B.

TCA9548A channels `0..5` and PCA9554 outputs `P0..P5` map to logical ports
`0..5`. Mux channels 6 and 7 are not supported ports, and firmware never selects
them in the six-slot build.

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

The high-current backplane/carrier interface remains entirely behind the upstream
backpack boundary.

| Signal | Direction | Notes |
| --- | --- | --- |
| `VDC_SLOT` | Backplane to carrier | Nominal 24 V module input. |
| `GND` | Shared | Power and signal return. |
| `SDA_SLOT_N` | Bidirectional | Slot-local SW3538 I2C data. |
| `SCL_SLOT_N` | Backplane to carrier | Slot-local SW3538 I2C clock. |
| `SLOT_N_PWR_SW` (Rev B) | Backplane to carrier | High-side switched nominal 24 V module input. |

Carrier SW3538 modules use the same I2C address; the main-backplane TCA9548A
provides one isolated segment per supported port. The PCA9554 power expander is
also on the upstream bus at `0x20`; its `P0..P5` outputs control slots 0 through 5.

## Logical Capacity and Board Revisions

PDCAN, shared crates, persistent records, and simulation support ports 0
through 7. Each firmware image embeds its hardware revision and supported-port
bitmap:

| Board | Supported bitmap | Ports |
| --- | ---: | --- |
| Backplane Backpack Rev A | `0x3F` | 0–5 |
| Backplane Backpack Rev B prototype | `0x3F` | 0–5, individually input-power gated |
| Future eight-port revision | `0xFF` | 0–7 |

Commands targeting an unsupported port return `UNSUPPORTED`/`INVALID_TARGET`; they
must not be reported as an absent module.
