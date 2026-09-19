---
title: Hardware
description: The backplane, carrier, and initial USB-C Power Delivery module.
weight: 2
badge: true
badge_text: BETA
resources:
  - src: _files/hardware-assemblies.webp
    name: hardware-assemblies
    title: Early hardware-assembly concept
    params:
      alt: Early hardware-assembly concept showing the backplane and installed modules
      caption: >-
        Initial render of the hardware assembly showing the backplane and installed modules.
  - src: _files/panel.webp
    name: fabrication-panel
    title: Fabrication-panel preview
    params:
      alt: Six-carrier, one-backplane fabrication panel
      caption: >-
        Fabrication-panel preview, not an assembled-system model. See
        [render provenance](_files/renders.json).
---

The current system has two custom PCB assemblies and one user-installed output
module. The backplane supplies shared infrastructure; each carrier owns its
local protection, measurement, control, and output module.

{{< figure name="hardware-assemblies" >}}

{{< figure name="fabrication-panel" >}}
