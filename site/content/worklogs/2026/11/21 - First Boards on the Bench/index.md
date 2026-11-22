---
title: 'First Boards on the Bench'
date: 2026-11-21
description: The first batch from PCBWay is on my desk, and I've got a J-Link adapter on order.
tags:
  - hardware
slug: '21-first-boards-on-the-bench'
draft: false
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

# The first boards have arrived!

In my [last worklog entry]({{< relref "/worklogs/2026/09/10 - First Physical Prototypes/index.md" >}}), I introduced the project and wrote that the first batch of boards was on order.

They did arrive! ... just hours before I left for vacation.
Now that I'm back, I finally have a chance to try them out and document how it goes.

{{< figure name="first-boards" >}}

## We Have Boards!

First, a _big_ thank you to PCBWay for covering part of the cost of these prototypes in exchange for trying their service and giving them feedback.

I have never had such a seamless experience with component sourcing before.

I didn't have to chase down any components myself, no consignment stock to manage and no endless back-and-forth with suppliers.
I sent them the list of parts, they told me how much it was going to cost and about how long it was going to take to get the parts and I approved the order... and then went on to other things while _they_ did all the work of sourcing and procuring the components.

It's hard to over-state particularly how valuable it is to have someone else take care of all the component procurement for you... especially when you've based your whole design around a relatively new (read: in short supply) microcontroller.

The boards themselves look excellent. The fabrication and assembly exceeded my expectations for a small prototype run, especially around the buck converters. I spent more time than I want to admit on that layout, balancing the electrical and thermal constraints against what I could reasonably ask someone to assemble. The least-bad compromise was to pack the components close together to keep the traces short and the thermal resistance low.

It's hard to convey just how _small_ these components are and how _close together_ they sit.

{{< figure name="pcbway-backplane" >}}

I worried that I was asking too much of the pick-and-place machines. Judging by the boards on my bench, they handled it just fine.

Also, look at that gold plating. 🤩


## Let's bring them up!

As is tradition, the first iteration _never_ goes perfectly.

### Getting the debugger connected

I'll admit it. I was so excited to see if I could talk to the MCU that I skipped straight past characterizing the power rails.
If I'd gotten the design right, the programmer could supply enough juice on the 3.3 V rail to power the MCU for that first check.

I reached for my J-Link and realized its breakout adapter was missing the 2×3 header my Tag-Connect probe uses.

Fortunately, designing an adapter is _almost_ a one-shot prompt with modern LLMs. :)

{{< figure name="jlink-swd" >}}

The adapter design lives in `hardware/accessories/jlink-swd`. 
While I wait for boards to come back, this mess of fiddly jumper wires does the trick as long as I'm careful.

{{< figure name="jank_wiring" >}}

If you're following along at home:

| Signal | Jumper color |
| ------ | ------------ |
| GND    | White        |
| nRESET | Orange       |
| SWCLK  | Purple       |
| SWDIO  | Blue         |
| 3.3 V  | Yellow       |

I wouldn't want to use this harness long term, but it got me my first answer.
I could talk to the MCU and verify that I didn't mess that part of the design up.

### Sorting out the LED orientation

Shortly after the “Your order is now being assembled” notification, PCBWay's support team sent me a photo.
They'd highlighted the same component on both boards:

{{< figure name="led_rotation_issue" >}}

For `$reasons` I still don't fully understand, the LED on each board was rotated 180°.
Its orientation looks right in the KiCad renders, and I haven't seen the same issue on any other part so ¯\_(ツ)_/¯.

It was a quick "nope, that should be the other way around, see this render from KiCad..." email exchange and the issue was promptly acknowledged by the support team and corrected in the assembly process.

### First power tests

Once I knew I could talk to the MCU, I breathed a little easier and went back to the power rails.

I may write more about the methodology later, but so far the buck modules behave more or less as I'd expected.
The 3.3 V logic rail came up first.
The 12 V rail took a little longer to settle, and I'm still not sure why.
That rail only powers the PC fans, so it's not a huge concern; a little ripple or overshoot there would be tolerable, too.

{{< figure name="power-rail-startup" >}}

I also wanted to see what happened to the 24 V input when the bench supply had a low current limit.
I captured the input and the 12 V output together on two unloaded starts, one with the limit set to 200 mA and one at 100 mA.

{{< figure name="power-input-startup" >}}

The datasheet gives formulas for estimating ripple and transient response, but you never _know_ until you measure.
I'd made a few compromises on component selection to simplify the BOM, so I was particularly keen to see how the logic rail looked on the scope.

There's more ripple on the 3.3 V rail than I'd like, though I can live with what I measured here.
I'll check again under load before deciding whether to add capacitance.

{{< figure name="power-output-ripple" >}}

I made the plots above from raw scope captures and saved a few screenshots along the way. In this early startup capture, the two time cursors are 400 µs apart.

{{< figure name="scope-startup-cursors" >}}

Here I was comparing the rising power rail with the `P_GOOD` signal.

{{< figure name="scope-input-ramp" >}}

And here's the burst I caught while debugging the LED. More on that one later.

{{< figure name="scope-pulse-burst" >}}
