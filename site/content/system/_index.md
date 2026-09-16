---
title: System
description: Technical documentation for the current Modular Rack Power design.
icon: gear
weight: 1
---

The core design principle of MPRS is to separate shared infrastructure from replaceable output modules:

1. A high current DC supply is wired to A backplane.
2. A consolidated backplane accepts the system DC input, measures aggregate
   current, powers the control electronics and fans, and distributes an
   unswitched DC bus to six slots.
2. Each carrier connects one slot to a power-output module. The initial carrier
   hosts an SW3538-based USB-C Power Delivery module.
3. The backplane and every carrier have their own STM32 controller and CAN-FD
   transceiver. CAN-FD is the only inter-board communication bus.

```text
DC input
   │
   ├── Backplane controller, current monitor, CAN-FD, and cooling
   │
   └── Shared VIN_BUS
          ├── Carrier 1 ── USB-C PD module
          ├── Carrier 2 ── USB-C PD module
          ├── ...
          └── Carrier 6 ── USB-C PD module
```

The second-generation backplane consolidates functions that previously lived
on separate backplane and controller boards. Older backpack-to-backplane I²C
designs remain in the repository as project history but are not the current
architecture.

## Current design targets

| Property | Current target |
| --- | --- |
| System/backplane nominal input | 24–48 V DC |
| Initial system input | Nominal 24 V DC |
| Current SW3538 carrier input limit | 30 V maximum; a new carrier revision is required for 48 V |
| Carrier slots | Six |
| Initial USB-C policy ceiling | 20 V, 5 A, 100 W per port |
| Working aggregate input budget | Approximately 30 A |
| Inter-board communication | CAN-FD |
| Backplane-local monitoring | INA237 over local I²C |
| Cooling | Two independently switched 3-wire fans |

There is no below-24 V nominal input specification. The current carrier is the
first revision's voltage limit, not the intended limit of the backplane. Never
fit a current-generation carrier to a backplane powered from 48 V.

The design does not presently switch or limit each backplane slot. Slot power
is present whenever the shared input bus is energized. Fault protection and
safe bring-up therefore remain hardware and system-integration concerns rather
than firmware guarantees.

For the underlying engineering record, see the repository's
[system overview](https://github.com/kquinsland/modular-rack-power/blob/main/docs/system-overview.md).

## Interfaces

### System DC input

The backplane accepts ground and `VIN_RAW` through a user-installed two-position
terminal. The initial population targets a nominal 24 V source. After the
aggregate current shunt, the rail becomes `VIN_BUS` and feeds the backplane
converters and all six slots.

### External CAN-FD

The external connection exposes ground, CAN low, and CAN high. Connector-side
ESD protection and a common-mode choke protect this boundary. A normally-open
120 Ω termination option is intended only for installations where the
backplane sits at a physical end of the CAN bus.

### Carrier slots

Each combined power-and-signal connector carries:

| Contact | Signal | Purpose |
| ---: | --- | --- |
| 1 | `VIN_BUS` | Unswitched power from the backplane |
| 2 | `GND` | Power and signal return |
| 3 | `CANH` | Shared CAN-FD high |
| 4 | `CANL` | Shared CAN-FD low |

Each carrier includes its own connector-side CAN protection. The backplane does
not duplicate an ESD network at every slot.

### Backplane-local interfaces

- I²C connects only the backplane STM32 and aggregate INA237 current monitor.
- Two 3-wire fan headers provide ground, switched 12 V, and tachometer input.
- The external DS18B20 header provides ground, 1-Wire data, and 3.3 V.
- A Tag-Connect SWD footprint provides target voltage, SWDIO, reset, SWCLK, and
  ground.

Consult the engineering
[interface tables](https://github.com/kquinsland/modular-rack-power/blob/main/docs/interfaces.md)
before relying on connector reference designators or fabricating hardware.

## Power and cooling

### Initial operating budget

| Item | Working value |
| --- | ---: |
| System/backplane input | Nominal 24–48 V |
| Initial population input | Nominal 24 V; current carrier maximum 30 V |
| Carrier slots | Six |
| Per-slot input budget | Approximately 4.7 A |
| Aggregate backplane target | Approximately 30 A |
| USB-C output policy | Up to 20 V, 5 A, 100 W |
| Aggregate current shunt | 1 mΩ |

The backplane is intended to support both 24 V and future 48 V carrier
generations. The current SW3538 carrier is the limiting factor and must not be
used on a 48 V bus; that requires a new carrier revision. The table's current
and power budgets apply to the initial 24 V population, not a qualified future
48 V/240 W configuration. Supply tolerance and low-line startup still need
validation; voltages below 24 V are not separate nominal system ratings.

At 30 A, the aggregate shunt develops about 30 mV and dissipates about 0.9 W.
The INA237 must use the corresponding narrow measurement range, and the sense
traces must connect to the shunt as a true Kelvin pair.

The backplane does not enforce per-slot current limits. Upstream interruption
and any per-slot fuse, eFuse, current-limiting, or bring-up strategy must be
resolved independently of software.

### Local rails and cooling

Separate LM5164 converters generate 3.3 V for the backplane logic and 12 V for
two fan channels. Both fans share the 12 V converter's 1 A capability even
though their power switches and tachometer inputs are independent. Continuous
load, simultaneous startup, and stall behavior require first-article testing
with the selected fans.

The estimated 11 W loss from each fully loaded initial USB-C module makes
forced-air cooling a design input, not an optional accessory. Actual module,
connector, shunt, copper, and enclosure temperatures must be measured before a
system rating is released.

See the engineering
[power budget](https://github.com/kquinsland/modular-rack-power/blob/main/docs/power-budget.md)
for conductor calculations and open protection questions.
