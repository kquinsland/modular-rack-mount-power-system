# Power Budget

Track the system and per-slot power assumptions here.

| Item                       | Value | Notes                                                    |
| -------------------------- | ----- | -------------------------------------------------------- |
| Backplane input voltage    | TBD   | Define nominal and max.                                  |
| Backplane input current    | TBD   | Include connector, copper, and fuse limits.              |
| Carrier slot voltage       | TBD   | Usually same as input unless converted on the backplane. |
| Carrier slot current       | TBD   | Must match connector, copper, and protection strategy.   |
| Number of carrier slots    | TBD   | Determines total backplane current.                      |
| USB-C PD module max output | TBD   | Per carrier.                                             |
| Controller 5 V input       | TBD   | WT32 plus all backplane WS2812B LEDs.                    |
| Controller 12 V input      | TBD   | Fan start/stall current and switching margin.            |
| WT32 3.3 V output budget   | TBD   | Controller logic plus every daisy-chained TCA9548A.      |

## Protection Checklist

- Input fuse or breaker.
- Per-slot fuse, eFuse, current limit, or zero-ohm bring-up option.
- Reverse polarity protection, if input connector allows it.
- TVS or surge strategy for the DC input.
- I2C pull-up placement and voltage domain.

## USB-C PD Modules

It turns out that the [SW3538 modules](https://github.com/happyme531/h1_SW35xx/issues/13#issuecomment-4361605621) use a non-standard configuration to reach their advertised "140W" spec; proprietary 20 V at 7 A mode.
Standard USB PD 3.0 tops out at 20 V at 5 A (100 W) which is already _plenty_ for my intended loads (they top out at around 70W!).

I don't have any cable/load that can even negotiate above 100W so we can treat this as the power ceiling.
To add an extra layer, I can limit them to 100W in software (probably...) and that will be plenty for my intended uses.

As the 100W figure is the "at the C port" figure, we must account for losses.
The datasheet's greater-than-95% figure is measured at a much lighter 12 V to 5 V, 25 W operating point, so it should not be used for the 24 V to 20 V full-load budget.

Assuming something less than 95% we get:

| Assumed efficiency | Input power | 24 V input current | Module power loss |
| -----------------: | ----------: | -----------------: | ----------------: |
|                92% |     108.7 W |             4.53 A |             8.7 W |
|                90% |     111.1 W |             4.63 A |            11.1 W |

I'll go with 90% efficiency for the budget, which is a bit pessimistic but gives some margin for the unknowns.
Each module will spit out about 11W which _will_ require a fan.

But the best part of this is that we now only need to target 4.7A for each of the 6 slots per backplane.
So now we only need to handle about 30A at 24V for the backplane which is a LOT BETTER than the ~40A I was dreading about before.

Detailed calculations, qualifications, and sources are in
[SW3538 USB-C/PD Module Power-Input and Thermal Budget](sw3538_usb_c_pd_power_budget.md).

## Prototype Backplane Copper Sizing

For the simplified backplane, assume a 30 A maximum shared input current,
2 oz external copper (nominally 70 µm or 2.756 mil), and a 10 °C conductor
temperature rise. The IPC-2221 relationship used by KiCad is:

`I = k × ΔT^0.44 × (W × H)^0.725`

For an external conductor, `k = 0.048`. Solving for width at 30 A gives
approximately 644.5 mil, or 16.37 mm, on one external layer. KiCad's own
[formula notes](https://gitlab.com/kicad/code/kicad/-/blob/10.0/pcb_calculator/tracks_width_versus_current_formula.md)
state that the model is valid only up to a 400 mil (10 mm) width, so that
single-layer result is an extrapolation and should be treated as a rough
engineering estimate.

The prototype therefore uses matching F.Cu and B.Cu rails in parallel. An
ideal 50/50 split is 15 A per layer, which requires 247.8 mil or 6.29 mm per
layer. The `POWER_30A` net class rounds this up to an 8.0 mm per-layer minimum
with 0.5 mm clearance. The same formula estimates 17.85 A per 8 mm external
conductor, or 35.7 A nominal combined capacity before accounting for current
sharing and local clearance voids.

Each 4.7 A slot branch requires approximately 1.27 mm on one 2 oz external
layer. Solid zone connections provide substantially more copper than that at
each XT30 pad. The paired rails must remain on both outer layers; any future
layer transition needs a separately engineered via array rather than a single
net-class-default via.
