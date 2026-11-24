---
title: 'A 3 A load and a firmware update over CAN'
date: 2026-11-23
slug: carrier-power-and-can-updates
draft: false
description: Measuring the carrier from supply to sink, getting the RH-02 PRO onto SocketCAN FD, and watching a trial image survive a reset.
tags: [hardware, firmware]
resources:
  - src: _files/current.webp
    name: current
    title: Three views of carrier input current
    params:
      alt: The INA237 reads progressively higher than the series meter, while the bench supply closely follows it.
      caption: Each point uses the final six telemetry samples at a load step and the 121GW readings received over the same interval.
  - src: _files/voltage.webp
    name: voltage
    title: Voltage at the USB port and load
    params:
      alt: The port stays around 20.1 V while voltage at the load falls to 19.58 V at 3 A.
      caption: The KM003C sits at the port. The BW600 reads at the far end of the cable and trigger assembly.
  - src: _files/hold.webp
    name: hold
    title: Three minutes at 2 A
    params:
      alt: During the 2 A hold the load heatsink warms to 43 degrees Celsius and the carrier current monitor reaches 23 degrees.
      caption: These sensors are on the electronic load and the carrier INA237. MOD1 itself had a heatsink but no temperature probe.
---

# The carrier gets some attention

I've spent the past few days getting the carrier board up and running.
The carrier is "the business end" of this whole setup so it's rather important to _thoroughly_ test its ability to report its input current accurately and verifying the readings against independent measurements.

So far, the results have been _mostly_ encouraging. It's a relief when the first prototypes work well enough that the next PCB revision only needs a few small changes.

## Following the current

I keep saying that I'll get around to a more detailed writeup of test methodology... and that might still happen.
For now, here's the hardware I've been using for the initial tests:

- [RD6006](https://hackaday.com/2020/03/16/review-the-riden-rd6006w-dc-power-supply-module/)
- [121GW](https://eevblog.store/products/121gw-multimeter)
- [POWER-Z KM003C](https://www.power-z.com/products/chargerlab-power-z-km003c)
- [ATORCH BW600](http://en.atorch.cn/ProDetail.aspx?ProID=12)

All of the above devices have some sort of USB/Serial/BT interface for logging or control and either partially or fully documented APIs.
If you can read between the lines, the programmatic control and some LLM written python _is_ the `TL;DR` for the test methodology.

Anyways.
The 121GW was placed in series with the 24 V supply from the RD6006 and that went straight into a carrier board.
On the USB side, the BW600 drew the load from the SW3538 module. The KM003C sat at the module's output so I could watch the USB PD negotiations and measure the current flowing to the sink.

What I _really_ wanted to see was how the carrier's INA237 compared with the other two instruments measuring its input current.

{{< figure name="current" >}}

The bad news is that there's a noticeable discrepancy between the INA237 reading and the other current measurement devices.
The good news is that it's pretty linear so if I can't correct this in hardware, it should be possible to apply a software correction to get accurate current readings and maybe "self-calibrate" in software.

{{< figure name="current" >}}

At the 3 A output step, the RD6006 and the series meter both read **2.661 A** going into the carrier.
The INA237 reported **2.835 A**.
Its raw shunt reading was 17.05 mV; the nominal 6 mΩ resistor should give 15.97 mV at that current.

One of the next tests to run is to measure directly across the shunt and repeat on a second carrier before changing the calibration.

Since I already have the data, this is what voltage loss looks like across the cable:

{{< figure name="voltage" >}}

At the highest step the KM003C measured **20.115 V**, and the BW600 sink drew **2.999 A at 19.580 V**.
That half-volt difference is across the whole measurement path which includes the cable, connectors and the USB-C/PD trigger accessory attached to the raw "DC IN" port on the BW600.

Not terrible.

I can't reliably/safely trigger more than 3 A with this setup (no eMarker cable handy) so that's where testing stopped.

One reason for instrumenting the hell out of this project is to keep a _very_ close eye on temperatures. The room was about 19 °C during the three-minute hold at 2 A.

{{< figure name="hold" >}}

The BW600 reported a temperature above ambient.
I don't know exactly where its probe sits, so I'd take the absolute reading with a grain of salt.
I'd expect the actual heatsink temperature to follow a similar trend, but I don't know whether the two curves would differ by a constant offset.

The carrier's INA237, on the other hand, barely warmed up during the hold.
That's good news for the monitor. I didn't have a temperature probe on MOD1 itself.

## Getting the RH-02 PRO onto CAN FD4

I've used the [Jhoinrch RH-02 PRO](https://www.amazon.com/clp/B0FSVNBKQ4) for most of my CAN work, and it's generally worked well for classic CAN. This is the first project where I've needed CAN FD, and I wanted to use it through Linux's native SocketCAN interface.

The [Elmue/CANable-2.5-firmware-Slcan-and-Candlelight project](https://github.com/Elmue/CANable-2.5-firmware-Slcan-and-Candlelight) describes its hardware testing this way.

> The firmware has been tested on the STM32G431 on the isolated adapters from MKS Makerbase and Jhoinrch up to 10 Mbaud

The project doesn't explicitly mention the `RH-02 PRO`, which tops out at 5 Mbaud, so I wasn't sure whether the firmware would work on mine.

It did!
My adapter uses an `STM32G431C8T6` and has a 25 MHz crystal.

I flashed [Elmü’s Jhoinrch Candlelight2.5 image](https://github.com/Elmue/CANable-2.5-firmware-Slcan-and-Candlelight/blob/5f2d69e64933b389f0ece4e752c9ecd54cb61c81/Binary%20Files/Candlelight2.5%20-%20Jhoinrch%20-%20STM32G431.dfu) via USB DFU.
Linux's built-in `gs_usb` driver then reported an MTU of 72 on the CAN interface.

With basic ping-pong firmware on the carrier, I captured a 16-byte request and its matching 16-byte reply. Bitrate switching was enabled, with **500 kbit/s for arbitration and 2 Mbit/s for data**.

With that working, I could finally try a firmware update over the bus. :)

## Swapping applications on the bench

I flashed a baseline application (version `0.1.0`) via the J-Link, then used `pdcan` to transfer a new application over CAN FD to the carrier's second partition.
Being able to do that in situ is **awesome**.
The interesting part was what happened when I reset the carrier.

- **Rollback.** I staged `0.1.1`, activated it, and queried the carrier. It reported `0.1.1`. I left the trial unconfirmed and reset the chip. On the next boot it reported `0.1.0` again, just as I'd hoped.
- **Confirmation.** The next trial included a few built-in health checks. Once the application could talk to the INA237, it passed those checks and marked itself as booted. I watched it confirm the trial, reset the chip, and queried the version again. This time it still reported `0.1.1`. :)

I still haven't tested signing or what happens if power drops mid-update. On the bench, though, I can stage an update over CAN FD, roll back an unconfirmed trial, and keep a confirmed image across a reset which is a **huge step forward** for tests and validation as the TagConnect is a bit fiddly and not fun to hold in place every time something needs to be updated.

Next up is finishing bring-up and characterization on the individual boards before mating them. After that, I want to try updating the carrier's configuration and firmware under load and see whether the rest of the system can keep doing its job through an update.
