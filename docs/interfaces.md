# Interfaces

## Controller Power Input

Current schematic connector: controller `J4`, 1x03 2.54 mm placeholder.

| Pin | Signal | Direction | Notes |
| --- | --- | --- | --- |
| 1 | `GND` | Input | Common return. |
| 2 | `+5V` | Input | Regulated WT32 and backplane-LED supply. |
| 3 | `+12V` | Input | Regulated fan supply. |

## Controller to Backplane I2C

Controller `J2` mates with the first backplane's `J10`. Backplane `J11` repeats
the same pinout for daisy-chaining.

| Pin | Signal | Direction | Notes |
| --- | --- | --- | --- |
| 1 | `GND` | Shared | Logic return. |
| 2 | `I2C_UP_SCL` | Controller to backplane | WT32 GPIO14. |
| 3 | `I2C_UP_SDA` | Bidirectional | WT32 GPIO32. |
| 4 | `+3V3` | Controller to backplane | WT32 3.3 V output; total load must be verified. |

## Controller to Backplane LEDs

Controller `J3` mates with the first backplane's `J8`. Backplane `J12` is the
LED-chain output to another backplane.

| Pin | Signal | Direction | Notes |
| --- | --- | --- | --- |
| 1 | `GND` | Shared | LED return. |
| 2 | `LED_DATA` / `LED_DATA_IN` | Controller to backplane | WT32 GPIO4 to first WS2812B DIN. |
| 3 | `+5V` | Controller to backplane | LED-chain power. |

## Controller Fan

Controller `J1` uses the standard 3-wire fan pinout.

| Pin | Signal | Direction | Notes |
| --- | --- | --- | --- |
| 1 | `GND` | Controller to fan | Fan return. |
| 2 | `FAN_12V_SW` | Controller to fan | High-side switched 12 V supply. |
| 3 | `FAN_TACH` | Fan to controller | Pulled up to 3.3 V; WT32 GPIO35. |

Fan supply PWM is driven from WT32 GPIO33. Firmware must use a low switching
frequency appropriate for supply-PWM control of a 3-wire fan and disregard tach
readings at zero or very low duty cycle.

## Backplane to Carrier Slot

Connector family: XT30-style or hybrid connector, exact part number TBD.

| Signal | Direction | Notes |
| --- | --- | --- |
| `VDC_SLOT` | Backplane to carrier | Main DC feed. Voltage and current rating TBD. |
| `GND` | Shared | Power and signal return. Final grounding strategy TBD. |
| `SDA_SLOT_N` | Bidirectional | I2C data for mux channel `N`. |
| `SCL_SLOT_N` | Backplane to carrier | I2C clock for mux channel `N`. |
| `SLOT_PRESENT_N` | Carrier to backplane | Optional. Add only if the connector or system controller needs detection. |
| `SLOT_EN_N` | Backplane to carrier | Optional. Add only if the carrier needs power/module enable control. |
| `SLOT_FAULT_N` | Carrier to backplane | Optional. Add only if carrier-side fault reporting is needed outside I2C. |

## I2C Addressing

Carrier DC/USB-C PD modules are assumed to have a fixed I2C address. The backplane mux gives each carrier its own downstream I2C segment so identical carrier modules do not collide.

## Naming

Use slot-indexed net names on the backplane, for example `SDA_SLOT_1`, `SCL_SLOT_1`, `VDC_SLOT_1`. On the carrier, use local names such as `SDA_MODULE`, `SCL_MODULE`, and `VDC_IN`.

The 2.54 mm pin headers in the current schematics are electrical/placement
placeholders. Select keyed production connectors before routing.
