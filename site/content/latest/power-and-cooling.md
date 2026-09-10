---
title: Power and cooling
description: Current power budgets, distribution assumptions, monitoring, and thermal constraints.
weight: 4
---

## Initial operating budget

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
