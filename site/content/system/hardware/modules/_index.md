---
title: Modules
description: Carrier-based and direct-connect power-output assemblies supported by the system.
weight: 2
---

Modules turn a backplane slot's shared DC and CAN-FD connection into a useful
power output. The initial design uses a reusable carrier, but future module
variants may connect directly without that carrier.

## Carrier

The [carrier]({{< ref "/system/hardware/modules/carrier/" >}}) mates with a
backplane slot and hosts a user-installed power-output module. It provides
local protection, measurement, and control. The initial configuration uses an
SW3538-based USB-C Power Delivery module.

See the carrier documentation for board previews, responsibilities, and the
operating limits of the current carrier/module combination.
