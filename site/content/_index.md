---
title: Modular Rack Power Supply System
description: Development journal and technical documentation repository for a modern, modular USB-C based power supply.

resources:
  - src: _files/hardware-assemblies.webp
    name: hardware-assemblies
    title: Early hardware-assembly concept
    params:
      alt: Early hardware-assembly concept showing the backplane and installed modules
      caption: >-
        Early mechanical concept image. A current assembly render will replace it as
        the release pipeline is completed.
---

Modular Rack Power Supply System (MRPS) is a 'creatively' named system for distributing DC power via USB-C/[Power Delivery](https://en.wikipedia.org/wiki/USB_hardware#USB_Power_Delivery) in a compact compute rack.

{{< figure name="hardware-assemblies" >}}

> [!WARNING]
> Everything about this project is still under active development.
> [Discussions](https://github.com/kquinsland/modular-rack-mount-power-system/discussions) about this project are welcome, but keep in mind that it is still in the early stages of development!

## What/Why

> [!INFO]
> If you're looking for _technical_ documentation about the system, see the [`system` section](./system/).

To make a _very_ long story short, [the power supply I built _years_ ago](https://karlquinsland.com/home-lab-consolidated-psu/) to power my Home Lab is no longer adequate for my current needs.

This project is a "do-over" for that earlier power supply and aims to address its limitations while incorporating some modern design principles and technologies like USB-C/Power Delivery.

## Goals

This project was started independently / before I knew about the
[HomeLab PDU V1 project from Shrike Lab](https://github.com/Shrike-Lab/HomeLab-PDU-V1) but it does target the same problem space even if this project has different goals.

In no particular order, the values and motivations for this project include:

- USB-C/PD is the future.
  - **Standards Compliance**: I intend to adhere to the USB-C/PD standards.
    - Only SPR/EPR profiles will be supported.
  - AliExpress has a wide variety of USB-C/PD adapters
    - It's trivial to adapt all the custom / proprietary barrel jacks, square shaped connectors and whatever "special" power  everything else into a _single_ standard connector.
  - Instead of a spaghetti mess of different cables and converters, a single consolidated AC->DC supply for greater efficiency and simplicity.

- Want to see how far I an push hardware design with the assistance of an LLM.

- I get to learn a few things / develop patterns and skills for future projects.

- A consolidated power supply must be modular and flexible.
  - Workloads are in production so redundancy is important, and a failure should be contained / a single dead port must not bring down the entire system!

- Modular design allows customization
  - Cost efficiency; spend $ where it matters and go for a cheaper alternative where possible.
    - Don't pay for a USB-C port that can deliver 100W if you only need 10W for that particular device.
    - If your power needs grow over time, you can add new modules or upgrade individual modules without replacing the entire system.
  - A modular system is inherently scalable.
    - If I do this right, it should be straight-forward to adapt this design to different form factors and port counts.

- Telemetry
  - I like data. I like graphs. I like knowing what my systems are doing.
  - A power supply that can't tell you how much power each load is using is unacceptable
  - A power supply that doesn't let me govern power to individual ports is unacceptable
    - This allows for better power management and prevents a single port from monopolizing the available power.
    - Also, remote "turn it off and on again" capability for troubleshooting and maintenance.

And a final note on inspiration:

I've been following the "DIYson lamp" project for a while now.
The guy behind it - [@StevenBennettMakes](https://www.youtube.com/@StevenBennettMakes) - recently posted [this video](https://www.youtube.com/watch?v=ipVFRH6TQfM) which he ends with:

> What I actually wanted was to be completely absorbed in an engaging and difficult, complicated project that felt kind of important.
> I wanted to be I wanted to kind of like transcend a hobby and to feel like a compulsion, like something I had to work on.
> ...
> I actually wanted Tony Stark's creative flow state.
> I basically just wanted to like capture that and take it for myself and have it.

I can understand that.

<!--more-->
