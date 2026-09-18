---
title: Gateway
description: Gateway board allows for remote monitoring and control of the Modular Rack Power system.
weight: 3
badge: true
badge_text: CONCEPT
resources:
  - src: _files/gateway-concept.webp
    name: gateway-concept
    title: ESP32 CAN-FD gateway concept
    params:
      alt: Concept render of a compact green gateway board with a shielded wireless module, printed antenna, and four-position screw terminal
      caption: AI-generated concept placeholder for an ESP32-based CAN-FD gateway; component placement and connector details are illustrative, not a finished board design.
---

{{< figure name="gateway-concept" >}}

> [!NOTE]
> This is purely a concept.
> The image above is an AI-generated illustration of the idea.
> The only reason I'm putting it here is because I have every intention of developing it further.

As mentioned in the [goals]({{% ref "/#goals" %}}) section, I like telemetry.
I have every intention of getting metrics from the various components in this system exported to some sort of time-series database for analysis and visualization.

This almost certainly will be a basic board with nothing more than an ESP32 and CAN-FD interface.

Why an ESP32 and not STM32 like the rest of the system?

Because I don't see the gateway as a _core_ part of the system.
It's more of a really really useful accessory.

At least initially, I will likely go with an [ESPHome](https://esphome.io/)-based solution because that makes it trivial to get telemetry data exposed to a time-series database via MQTT or HTTP and also makes [HomeAssistant](https://www.home-assistant.io/) integration seamless.
