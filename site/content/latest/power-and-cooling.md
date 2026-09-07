---
title: Power and cooling
description: Current power budgets, distribution assumptions, monitoring, and thermal constraints.
weight: 4
---

## Initial operating budget

| Item | Working value |
| --- | ---: |
| Backplane input | Nominal 24 V |
| Carrier slots | Six |
| Per-slot input budget | Approximately 4.7 A |
| Aggregate backplane target | Approximately 30 A |
| USB-C output policy | Up to 20 V, 5 A, 100 W |
| Aggregate current shunt | 1 mΩ |

At 30 A, the aggregate shunt develops about 30 mV and dissipates about 0.9 W.
The INA237 must use the corresponding narrow measurement range, and the sense
traces must connect to the shunt as a true Kelvin pair.

The backplane does not enforce per-slot current limits. Upstream interruption
and any per-slot fuse, eFuse, current-limiting, or bring-up strategy must be
resolved independently of software.

## Local rails and cooling

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
