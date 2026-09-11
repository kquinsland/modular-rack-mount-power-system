---
title: Firmware
description: Placeholder for the firmware redesign that follows Gen-2 hardware stabilization.
weight: 3
badge: true
badge_text: Planned
---

Firmware documentation is intentionally deferred.

The firmware currently present in the repository does **not** describe the
planned second-generation implementation and must not be used as an
authoritative guide to system behavior, protocol, safety policy, or pin
allocation.

This section will be replaced after the Gen-2 hardware design stabilizes. The
future documentation is expected to cover:

- Backplane and carrier responsibilities.
- CAN-FD protocol and node identity.
- Power sequencing, fault handling, and recovery.
- Current, voltage, fan, and temperature telemetry.
- Boot, update, configuration, and diagnostic procedures.
- Hardware-specific builds and validation.

Until then, hardware protection must remain safe without relying on unfinished
firmware.
