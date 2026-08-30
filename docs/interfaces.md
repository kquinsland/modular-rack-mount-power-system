# Consolidated Backplane Interfaces

This document describes the active second-generation backplane hardware in
`hardware/boards/backplane-prototype`. The former backpack-to-backplane I2C
boundary is superseded; the controller and all six slots now live on one PCB.

## DC Input

Backplane `J5` uses DORABO `DB910-6.35-2P-GN-S`, LCSC `C395872`.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Input | Common power and signal return. |
| 2 | `VIN_RAW` | Input | Nominal 24 V for the first population. |

`VIN_RAW` passes through the 1 mOhm high-side shunt `RSH1` to `VIN_BUS`. The
INA237 measures this drop through 10 ohm/100 nF input filtering; its PCB sense
connections must be Kelvin-routed directly to the shunt pads.

## External CAN-FD

Backplane `J2` is a generic three-position screw-terminal symbol. Its exact
mechanical series and footprint are intentionally deferred until PCB placement.

| Pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | Shared | CAN reference/return. |
| 2 | `CANL_EXT` | Bidirectional | External CAN low. |
| 3 | `CANH_EXT` | Bidirectional | External CAN high. |

`D1` provides connector-side CAN ESD protection and `L3` is the boundary common-
mode choke. `SJ1` enables `R17`, the normally-open 120 ohm termination, only
when the backplane is installed at a physical end of the CAN bus.

The TCAN3413 standby input is tied low, so the transceiver is permanently
active whenever 3.3 V is present. MCU-controlled standby is not required.

## Carrier Slots

The six carrier slots use the carrier-authoritative AMASS
`XT30U(2+2)-F.G.B`, LCSC `C30170181`, footprint and pinout.

| Slot | Reference |
| ---: | --- |
| 1 | `J1` |
| 2 | `J3` |
| 3 | `J4` |
| 4 | `J6` |
| 5 | `J7` |
| 6 | `J8` |

| Physical pin | Signal | Direction | Notes |
| ---: | --- | --- | --- |
| 1 | `VIN_BUS` | Backplane to carrier | Unswitched high-current slot supply. |
| 2 | `GND` | Shared | Power and CAN return. |
| 3 | `CANH_BUS` | Bidirectional | Shared CAN-FD high. |
| 4 | `CANL_BUS` | Bidirectional | Shared CAN-FD low. |

There is no per-slot backplane ESD network. The backplane protects its external
CAN connector, and each carrier retains its own connector-side CAN protection.
The PCB layout must implement a short shared CAN trunk with short slot stubs.

## Local I2C

`I2C_SDA` and `I2C_SCL` connect only the STM32C092GCU6 and INA237 on the
backplane. They use 2.2 kohm pull-ups to 3.3 V. No I2C signal reaches a carrier
slot, and the old TCA9548A mux, PCA9554 expander, slot pull-ups, and slot I2C ESD
parts are not used.

## Fans

`J9` and `J11` use KiCad's vertical 2.54 mm 3-wire fan-header footprint. The
exact purchasable header MPN is not yet selected and must be checked against
that footprint before quoting. Each connector has an independent high-side
supply-PWM switch and tach input.

| Pin | `J9` signal | `J11` signal | Notes |
| ---: | --- | --- | --- |
| 1 | `GND` | `GND` | Fan return. |
| 2 | `FAN_12V_SW` | `FAN2_12V_SW` | Independently switched 12 V from Q1 or Q3. |
| 3 | `FAN_TACH` | `FAN2_TACH` | Open-collector tach input with a 10 kohm pull-up to 3.3 V. |

The interfaces support 3-wire fans only. They do not provide the fourth control
wire used by 4-wire PWM fans. Both connectors share the 1 A LM5164 12 V rail;
combined continuous current, simultaneous startup, and stall behavior require
first-article validation.

## External Temperature Sensor

`J12` is a standard vertical 2.54 mm 1x3 header for an externally powered
DS18B20:

| Pin | Signal | Notes |
| ---: | --- | --- |
| 1 | `GND` | Sensor return. |
| 2 | `DS18B20_DATA` | 1-Wire DQ to STM32 PB8. |
| 3 | `+3V3` | External sensor supply. |

`R24` is a 4.7 kohm pull-up from DQ to 3.3 V. This is the three-wire external-
supply arrangement; no parasite-power strong-pull-up circuit is provided. Any
cable-end bypass capacitor should be placed next to the sensor.

## SWD

`J10` is a Tag-Connect `TC2030-IDC-NL` footprint with this schematic pinout:

| Pin | Signal |
| ---: | --- |
| 1 | `+3V3` target reference |
| 2 | `SWDIO` |
| 3 | `NRST` |
| 4 | `SWCLK` |
| 5 | `GND` |
| 6 | Not connected |

## Local Power Rails

Separate LM5164DDAR converters generate 3.3 V and 12 V from `VIN_BUS`. Both
use PSPMAA0805-101M-ANP 100 uH inductors, but their feedback, RON,
ripple-injection, and output-capacitor networks are rail-specific.
