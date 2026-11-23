---
title: 'A 3 A load and a firmware update over CAN'
date: 2026-11-23
slug: carrier-power-and-can-updates
draft: true
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

# Carrier gets some attention

The past few days have been focused on getting the carrier board up and running.
The carrier is "the business end" of this whole setup so it's rather important to _thoroughly_ test its ability to report its input current accurately and verifying the readings against independent measurements.

From the early testing, the results have been _mostly_ positive and encouraging.
It's such a relief to have your first prototype articles _mostly_ work as expected and to come away with only a short list of _small_ revisions to make to the next PCB instead of facing major redesigns.

## Following the current

I keep saying that I'll get around to a more detailed writeup of test methodology... and that might still happen eventually. 
For now, just a quick rundown of the hardware I used for the tests and some of the initial results.

Equipment:

- [RD6006](https://hackaday.com/2020/03/16/review-the-riden-rd6006w-dc-power-supply-module/)
- [121GW](https://eevblog.store/products/121gw-multimeter)
- [POWER-Z KM003C](https://www.power-z.com/products/chargerlab-power-z-km003c)
- [ATORCH BW600](http://en.atorch.cn/ProDetail.aspx?ProID=12)

All of the above devices have some sort of USB/Serial/BT interface for logging or control and that's a _big_ component to the test methodology!

Anyways.
The 121GW was placed in series with the 24 V supply from the RD6006 and that went straight into a carrier board.
The BW600 was connected to the output of the SW3538 module to draw the load and the KM003C sat between them and monitored the PD negotiations and current flowing to the sink.

I am _really_ curious about the INA237 on the carrier and how it measures compared to the several other current measurement devices in the setup.

The bad news is that there's a noticeable discrepancy between the INA237 reading and the other current measurement devices.
The good news is that it's pretty linear so if I can't correct this in hardware, it should be possible to apply a software correction to get accurate current readings.

In _theory_ this could be "self-calibrating"; one could cross-check the INA237 on the _carrier_ with the INA237 on the backplane and use the current measured by the SW3538 module as a reference to calibrate or correct the carrier INA237 readings in software but a lot more testing needs to be done first.

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

I can't reliably/safely trigger more than 3 A with this setup so that's where testing stopped.

One of _the_ reasons for instrumenting the hell out of this project is to keep a _very_ close eye on thermals.
Knowing that it was about 19 °C ambient when running the tests, this is what the INA237 and the BW600 both reported for their temperature:

{{< figure name="hold" >}}

I don't know _where_ the temperature probe is on the BW600 but it seems to be higher than ambient so take the above graph with a grain of salt.
I do expect that the "true" value would also be linear if plotted, I just don't know if this curve is offset from the actual temperature by a constant amount.

In any case, almost no temperature change in the INA237 is good news.

## Getting the RH-02 PRO onto CAN FD4


I've used the [`Jhoinrch RH-02 PRO`](https://www.amazon.com/clp/B0FSVNBKQ4) for most CAN related things in the past and it has generally worked well for classic CAN.
This is the first project where I've needed to use it with CAN FD and ideally with _native_ OS support rather than some convoluted userspace driver / serial port workaround.

The [`Elmue/CANable-2.5-firmware-Slcan-and-Candlelight` repo](https://github.com/Elmue/CANable-2.5-firmware-Slcan-and-Candlelight) says:

> The firmware has been tested on the STM32G431 on the isolated adapters from MKS Makerbase and Jhoinrch up to 10 Mbaud

But it doesn't explicitly mention the RH-02 PRO... which only goes up to 5 Mbaud so it wasn't clear if it would work.

To make a very long story short, **it is possible** to flash the RH-02 PRO with firmware that supports CAN FD and is recognized by the OS natively.

My devices uses `STM32G431C8T6` and it does feature a 25 MHz crystal.
Flashing [Elmü’s Jhoinrch Candlelight2.5 image](https://github.com/Elmue/CANable-2.5-firmware-Slcan-and-Candlelight/blob/5f2d69e64933b389f0ece4e752c9ecd54cb61c81/Binary%20Files/Candlelight2.5%20-%20Jhoinrch%20-%20STM32G431.dfu) via USB DFU made Linux’s built-in `gs_usb` driver advertise MTU 72.

With a very basic ping/pong firmware flashed to the carrier, I captured a 16-byte request and its matching 16-byte reply with bitrate switching enabled: **500 kbit/s arbitration, 2 Mbit/s data**.

And with a working CAN FD adapter, it's time to work on "ota" updates :).

## Swapping applications on the bench

The testing methodology isn't super important here, but the key point is that I can reliably stage and confirm updates over CAN FD.

Starting with a baseline (version `0.1.0`) flashed via the J-Link, then using the `pdcan` tool to transfer a new application over CAN FD to the second partition.

**Awesome**.  It should be no trouble at all to make firmware updates easy to do in situ!

After the basic tests, I proceeded to verify both rollback and confirmation scenarios:

- **Rollback:** The carrier staged `0.1.1`, accepted activation and answered as 0.1.1. I left that trial 'unconfirmed' and then reset the chip. After it booted, it was responding with version `0.1.0` again so the rollback worked as expected.
- **Confirmation:** A few built-in health-checks were shipped with a follow up trial; if the update could talk to the INA237 it would pass those checks and then marked itself booted. I watched that happen and then reset the chip and re-queried it's version after boot; 0.1.1 was reported. :)

I still haven't done anything with signing or power-cut resilience but the basic OTA update mechanism over CAN FD is working reliably on the bench.

The plan is to finish the basic/isolated article bring-up and characterization before doing integration work.
Not _too long_ after that will come OTA tests while under load to ensure that the carrier's configuration and firmware can be updated while the system is actively operating, ideally without interrupting normal operation.