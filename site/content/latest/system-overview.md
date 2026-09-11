---
title: System overview
description: How the backplane, carriers, power distribution, and control interfaces fit together.
weight: 1
---

Modular Rack Power separates shared infrastructure from replaceable output
modules:

1. A consolidated backplane accepts the system DC input, measures aggregate
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

The backplane and carriers communicate directly on the shared CAN-FD trunk.
Local I²C buses end on their own PCB; there is no inter-board I²C bus or control
proxy.

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
| Local status | One NeoPixel on each backplane and carrier |
| Firmware updates | Host directly to one node over CAN; bootloader via SWD |

There is no below-24 V nominal input specification. The current carrier is the
first revision's voltage limit, not the intended limit of the backplane. Never
fit a current-generation carrier to a backplane powered from 48 V.

The design does not presently switch or limit each backplane slot. Slot power
is present whenever the shared input bus is energized. Fault protection and
safe bring-up therefore remain hardware and system-integration concerns rather
than firmware guarantees.

For the underlying engineering record, see the repository's
[system overview](https://github.com/kquinsland/modular-rack-power/blob/main/docs/system-overview.md).
