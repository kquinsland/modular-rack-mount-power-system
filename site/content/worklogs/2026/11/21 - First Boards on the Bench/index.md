---
title: 'First Boards on the Bench'
date: 2026-11-21
description: The first batch from PCBWay is on my desk, and I've got a J-Link adapter on order.
tags:
  - hardware
slug: '21-first-boards-on-the-bench'
draft: true
resources:
  - src: _files/power-rail-startup.webp
    name: power-rail-startup
    title: Logic and fan rails at turn-on
    params:
      alt: Startup traces at TP3 and TP4 show the logic rail settling before the fan rail.
      caption: >-
        The 3.3 V logic rail came up ahead of the 12 V fan rail. Over the final
        6 ms of this unloaded capture, they averaged 3.30 V and 12.00 V.
  - src: _files/power-input-startup.webp
    name: power-input-startup
    title: 24 V input and 12 V rail at startup
    params:
      alt: Two plots show the J8 input and TP4 fan rail rising at each bench supply current limit.
      caption: >-
        Two unloaded starts reached nominal voltage with the bench supply set to
        200 mA and 100 mA current limits. The traces show input and output voltage.
  - src: _files/power-output-ripple.webp
    name: power-output-ripple
    title: Output ripple at C4 and C10
    params:
      alt: C4 shows a repeating ripple waveform, while the C10 trace stays close to the local-ground measurement.
      caption: >-
        With a 20 MHz bandwidth limit, C4 on the 3.3 V rail measured 79.6 mV
        peak-to-peak. C10 on the 12 V rail measured 14.9 mV peak-to-peak,
        equal to its local-ground control in this capture.
  - src: _files/board01.webp
    name: first-boards
    title: First boards on the bench
    params:
      alt: A long backplane and two smaller boards laid out on a wooden workbench.
      caption: The backplane and smaller boards laid out after unpacking.
  - src: _files/issue01.webp
    name: led_rotation_issue
    title: LED orientation during assembly
    params:
      alt: An orange box marks the side-view LED beside the pin 1 label.
      caption: PCBWay highlighted the LED orientation in this assembly photo.
  - src: _files/jank01.webp
    name: jank_wiring
    title: Temporary SWD wiring
    params:
      alt: Colored jumper wires join the debug probe adapter to a gray ribbon cable.
      caption: The jumper harness used for the first SWD connection.
  - src: _files/jlink-swd.webp
    name: jlink-swd
    title: J-Link to Tag-Connect adapter
    params:
      alt: A 3D rendering shows the 20-pin socket, keyed ribbon header, and labeled signal taps.
      caption: The adapter routes the J-Link debug signals to the Tag-Connect ribbon cable.
  - src: _files/pcbway02.webp
    name: pcbway-backplane
    title: Backplane assembly detail
    params:
      alt: Two inductors marked 101 sit beside the PCBWay silkscreen on the green backplane.
      caption: The two regulator sections on the assembled backplane.
  - src: _files/scope-20261121-120402.webp
    name: scope-startup-cursors
    title: Startup capture with timing cursors
    params:
      alt: Yellow and cyan traces rise from their initial levels beside the vertical time cursors.
      caption: The Rigol cursor readout shows a 400 µs interval on this early startup capture.
  - src: _files/scope-20261121-151802.webp
    name: scope-input-ramp
    title: Input ramp and logic transition
    params:
      alt: The yellow input trace ramps upward as the cyan trace steps high.
      caption: The two channels during startup at 10 ms per division.
  - src: _files/scope-20261121-154055.webp
    name: scope-pulse-burst
    title: Pulse burst on the scope
    params:
      alt: A short train of positive pulses appears on channel 2 between two low intervals.
      caption: A single burst captured during LED data probing at 5 µs per division.
---

# Update: First Boards Arrived

The [last "work-log" entry]({{< relref "/worklogs/2026/09/10 - First Physical Prototypes/index.md" >}}) was a brief introduction to the project and an announcement of the first batch of boards being on order.

Well, the first set of prototype boards have arrived!

They arrived just hours before heading out for some planned vacation/time off so they've been sitting on my desk until recently... but now I finally have a chance to start examining them, do some initial testing, and document the process.

{{< figure name="first-boards" >}}

## We Have Boards!

First, a _big_ thank you to PCBWay for chipping in on the production of these prototype boards.
They sponsored some of the costs for these prototype boards in exchange for me giving their service a try and providing feedback.

To start; their sourcing team was incredibly thorough and diligent in tracking down all the necessary components for this build.

I didn't have to chase down any components myself, no consignment stock to manage and no endless back-and-forth with suppliers.
I sent them the list of parts, they told me how much it was going to cost and about how long it was going to take to get the parts and I approved the order... and then went on to other things while _they_ did all the work of sourcing and procuring the components.

It's hard to over-state particularly how valuable it is to have a sourcing team that takes care of all the component procurement for you... especially when you've based your whole design around a relatively new (read: in short supply) microcontroller.

Second, the boards came out looking excellent.
The quality of the PCB fabrication and assembly was impressive and exceeded my expectations for a small batch of prototype boards.

To make a very long story short, I spent more time than I want to admit trying to get the layout for the buck converters right.
Essentially, I was trying to balance the thermal and electrical constraints while keeping the layout compact and manufacturable.
The least-bad compromise between several competing constraints was to place components as close together as possible, minimizing trace lengths and thermal resistance above all else.

I can't stress enough how a) _small_ these components are and b) _close together_ they are...

{{< figure name="pcbway-backplane" >}}

I was worried that this might be asking too much of the Pick-n-Place machines but it seems they handled it without any issues.


Also, look at that gold plating. 🤩


## Let's do bring up!

With both free time and boards in hand, it was finally time to start the bring-up process!
As is tradition, the first iteration _never_ goes perfectly.
Here's a quick rundown of some issues I encountered during the initial bring-up.

### Wrinkle, the first

I'll admit it; I was too excited to connect the debugger and see if I could talk to the MCU that I completely skipped over characterizing the power rails.
After all, if I did things right, the programmer will supply enough juice on the 3v3 rail to power the MCU up without needing an external power source and that would be enough to at least get the initial bring-up going.

I reached for my J-Link and realized that the breakout/adapter that it shipped with did not have the 2x3 header that the tag-connect probe uses.

Fortunately, fixing this  is _almost_ a one-shot prompt with modern LLMs :).

{{< figure name="jlink-swd" >}}

The `hardware/accessories/jlink-swd` directory has the details for the adapter.
It was sent off for fabrication and it should be here shortly.

While I wait for the adapter, as long as I am careful, this mess of fiddly jumper wires will suffice for the initial bring-up.

{{< figure name="jank_wiring" >}}


If you're trying to follow along at home:


| Pin | Color |
|-----|-------|
| GND   | White |
| nRESET   | Orange |
| SWCLK   | Purple |
| SWDIO   | Blue  |
| 3v3   | Yellow|

It's certainly not pretty wiring or even a good idea / long-term solution, but it gets the job done for now.
And because it gets the job done, I was able to confirm that I can talk to the MCU successfully.

Nothing too exciting yet; I have some hardware/time constraints that prevent me from doing a _full_ test at this stage but some of the basic checks have passed successfully.


### Wrinkle, the second

Shortly after getting the "Your order is now being assembled" notification, PCBWay's support team sent me a photo of the assembled boards, highlighting a minor issue with one of the components on both boards.

{{< figure name="led_rotation_issue" >}}

For `$reasons` that I still don't fully understand _one_ component on both boards was rotated 180 degrees.
It appears fine in KiCad/renders and no other part has this rotation issue so it seems to be an isolated case.

It was a quick "that should be the other way around, see this render from KiCad..." email exchange and the issue was promptly acknowledged by the support team.


### First tests

After confirming that I could talk to the MCU, I felt a sense of relief and moved on to testing the power rails so I could move on to more comprehensive system tests.

I may write more about the methodology later but for now, the initial results show that the buck modules are functioning - more or less - as expected.
I'm not _exactly_ sure why the 12v rail takes a bit longer to stabilize compared to the other rails, but it does get there eventually.
It's also exclusively for PC fans so a bit of ripple and/or overshoot / general delay isn't the end of the world.

{{< figure name="power-rail-startup" >}}

I was testing what happens to the input voltage during startup, especially with low current limits.

{{< figure name="power-input-startup" >}}

The data sheet did have some good formulas for estimating the expected ripple and transient response but you never _know_ until you measure it yourself.
I was a bit anxious about this as: a) power rails are important for the stability of the system, and b) I had made some compromises in the component selection to simplify the BOM.

There's more ripple on the 3v3 rail than I'd like, but it's still within acceptable limits for my application.
I will keep a close eye on this once it's loaded down and may add a bit more capacitance if necessary.

{{< figure name="power-output-ripple" >}}

The renders above come from raw data gathered through the oscilloscope.
I did take a few screenshots just after some positive test results came back:

{{< figure name="scope-startup-cursors" >}}

Power rail versus the `P_GOOD` signal:

{{< figure name="scope-input-ramp" >}}

Debugging the LED; more to come on this later.

{{< figure name="scope-pulse-burst" >}}


