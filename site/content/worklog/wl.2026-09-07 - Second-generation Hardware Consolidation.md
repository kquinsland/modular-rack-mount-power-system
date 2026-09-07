---
title: Second-generation hardware consolidation
date: 2026-09-07
description: The controller and six-slot distribution are moving onto one backplane while each carrier takes ownership of its local power path.
tags:
  - hardware
  - backplane
  - carrier
slug: 2026-09-07-second-generation-hardware-consolidation
draft: false
---

The second generation of Modular Rack Power now has a much clearer boundary
between the shared backplane and each removable carrier.

The backplane is being consolidated into a single six-slot board. It owns the
system DC input, aggregate current measurement, CAN-FD connection, local
controller, two fan channels, and external temperature-sensor interface. This
supersedes the older architecture where a separate controller backpack reached
slot peripherals over I²C.

Each carrier now owns its local controller, CAN-FD transceiver, protected power
path, telemetry, and Power Delivery module interface. A slot needs only the
shared DC bus, ground, CAN high, and CAN low. That makes the electrical boundary
smaller and keeps fixed-address module control local to the carrier.

The initial operating target remains deliberately conservative: nominal 24 V
input and no more than 20 V, 5 A, or 100 W from each USB-C port. The eventual
48–50 V and 240 W-per-slot direction is still an experiment that will require
new module review and first-article electrical and thermal characterization.

The consolidated schematic is in place, but the PCB is still a work in
progress. High-current copper, component placement, regulator loops, the CAN
trunk, grounding, cooling, and test access all need review before fabrication.
The next useful milestone is a board that passes those checks and is ready for
a controlled first-article build.

Firmware will be redesigned after this hardware stabilizes. Existing firmware
in the repository should be treated as project history, not as the behavior of
the second-generation system.
