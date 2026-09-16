---
title: 'First Physical Prototypes'
date: 2026-09-10
description: After a lot of re architecture, the second-generation design is now ready for first-article prototypes.
tags:
  - hardware
slug: '10-first-physical-prototypes'
aliases:
  - /worklogs/2026-09-10-first-physical-prototypes/
draft: false
resources:
  - src: _files/early_prototype_01.webp
    name: early_prototype_01
    title: Early enclosure prototype
    params:
      alt: CAD view of the open rack enclosure with a power supply and a row of carrier modules
      caption: Early enclosure layout with the power supply and carrier modules.
  - src: _files/early_prototype_02.webp
    name: early_prototype_02
    title: Early rack layout
    params:
      alt: CAD view of two computers mounted above the power enclosure, with colored lines indicating cable routes
      caption: Early rack layout showing the computers above the power enclosure and proposed cable routing.
---

Journaling about progress on long-term projects is not new to me... but doing it publicly, in the repo is. Let's see how this goes!

---

Please see {{< ref "system/_index.md" >}} for a high-level overview of the goal/purpose, and architecture.

After some early and initial feedback on prototypes...
Lots of contention between initial goals, future plans, and trying to keep things simple but powerful and cheap... all at the same time.

//TODO: multi axis graph to illustrate the tradeoffs/tension?

Finally a nice mid-point that should make for a usable v1.0 system that can be extended somewhat w/ software.

Two backplanes are about the right number for a 1U rack.

{{< figure name="early_prototype_01" >}}

{{< figure name="early_prototype_02" >}}

A few limiting factors are keeping things at the ~100W/port level.
Highly motivated by telemetry; I want to see what's going on / measure all the things!
Modular so a part can be swapped out if it fails or if a new module is developed.
Individual port control so I can remotely reboot a node as needed.


It owns the
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

## PCBWay

// TODO: brief "yep, they're sponsoring..."


// TODO: some of the early assembly and testing notes.
