# Interfaces

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
