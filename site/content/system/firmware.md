---
title: Firmware
description: Generation-2 node firmware, host tooling, and CAN application updates.
weight: 5
badge: true
badge_text: BETA
---

Every backplane and carrier is an independent STM32C092GCU6 CAN-FD node. The
firmware workspace has separate backplane and carrier applications and a common
Embassy Boot bootloader. Earlier muxed-port firmware is not supported.

The current implementation establishes the Generation-2 contracts and safety
boundaries:

- Direct Node-ID discovery, commissioning, control, state, and telemetry.
- Capability-described backplane and carrier profiles, including `live` or
  `interrupt` firmware-update impact.
- Backplane support for aggregate INA237 monitoring, two fan channels,
  DS18B20 temperature sensing, and one status NeoPixel.
- Current SW3538 carrier support for local output control, INA237 monitoring,
  PD status/control, user input, and a status NeoPixel.
- Host-to-node application update messages with staged SHA-256 verification,
  explicit activation, and Embassy Boot trial/rollback foundations.
- A `pdcan` host utility whose Clap command tree is available to automated
  clients through `pdcan schema`.

The current SW3538 profile declares `interrupt`: staging may occur without
resetting it, but activation deliberately disables the output and requires an
explicit operator acknowledgement. A future profile may declare `live` only
after hardware-in-the-loop tests prove service continuity through activation,
trial boot, failure, and rollback.

CAN updates cover application images only. The host updates each target
directly; a backplane never updates a carrier. Bootloader programming and
recovery remain SWD operations.

The protocol, drivers, state machines, artifact format, simulator, CLI, and
minimal embedded images are implemented and tested. Full peripheral task
integration and first-article hardware/rollback validation remain in progress,
so hardware safety must not depend on unvalidated firmware behavior.

See the repository's
[firmware documentation](https://github.com/kquinsland/modular-rack-power/tree/main/firmware/docs/pdcan)
for the engineering contract and validation checklist.
