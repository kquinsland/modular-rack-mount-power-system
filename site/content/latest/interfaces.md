---
title: Interfaces
description: External DC and CAN-FD connections, carrier slots, cooling, temperature sensing, and debug.
weight: 3
---

## System DC input

The backplane accepts ground and `VIN_RAW` through a user-installed two-position
terminal. The initial population targets a nominal 24 V source. After the
aggregate current shunt, the rail becomes `VIN_BUS` and feeds the backplane
converters and all six slots.

## External CAN-FD

The external connection exposes ground, CAN low, and CAN high. Connector-side
ESD protection and a common-mode choke protect this boundary. A normally-open
120 Ω termination option is intended only for installations where the
backplane sits at a physical end of the CAN bus.

## Carrier slots

Each combined power-and-signal connector carries:

| Contact | Signal | Purpose |
| ---: | --- | --- |
| 1 | `VIN_BUS` | Unswitched power from the backplane |
| 2 | `GND` | Power and signal return |
| 3 | `CANH` | Shared CAN-FD high |
| 4 | `CANL` | Shared CAN-FD low |

Each carrier includes its own connector-side CAN protection. The backplane does
not duplicate an ESD network at every slot.

## Backplane-local interfaces

- I²C connects only the backplane STM32 and aggregate INA237 current monitor.
- Two 3-wire fan headers provide ground, switched 12 V, and tachometer input.
- The external DS18B20 header provides ground, 1-Wire data, and 3.3 V.
- A Tag-Connect SWD footprint provides target voltage, SWDIO, reset, SWCLK, and
  ground.

Consult the engineering
[interface tables](https://github.com/kquinsland/modular-rack-power/blob/main/docs/interfaces.md)
before relying on connector reference designators or fabricating hardware.
