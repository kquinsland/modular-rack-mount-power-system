---
title: Carrier
description: Per-slot control, protection, telemetry, CAN-FD, and output-module interface.
weight: 1
---

One carrier mates with each backplane slot and hosts one user-installed power
output module. It receives unswitched DC plus CAN-FD from the backplane and
generates its own 3.3 V housekeeping rail.

![Placeholder for the carrier PCB render](/images/pcbs/carrier.png)

_Placeholder illustration. Release tooling will replace this image with the
current PCB render._

## Responsibilities

- Switch the high-current branch to the output module under local MCU control.
- Limit inrush and handle short-circuit, overload, undervoltage, and
  overvoltage conditions through dedicated hot-swap hardware.
- Measure module input voltage, current, and power with a local INA237.
- Communicate with the backplane over CAN-FD.
- Control the attached module through carrier-local I²C when required.
- Drive a local addressable status LED.
- Default the module power path off while the MCU is unpowered or resetting.

The carrier's housekeeping electronics connect ahead of the module current
shunt. Module telemetry therefore excludes the carrier controller, CAN
transceiver, and status LED consumption.

The initial population is operated as a nominal 24 V, 100 W system. Although
some component choices support future higher-voltage experiments, 48–50 V and
240 W operation is not a released capability.

See the detailed
[carrier hardware handoff](https://github.com/kquinsland/modular-rack-power/blob/main/hardware/boards/carrier/hardware-handoff.md)
for component-level design inputs and unresolved validation work.
