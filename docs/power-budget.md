# Power Budget

Track the system and per-slot power assumptions here.

| Item | Working value | Notes |
| --- | ---: | --- |
| Backplane nominal input | 24 V | Final supply tolerance remains to be documented. |
| Backplane sustained load target | Approximately 30 A | Six ports near 4.7 A input plus system margin. |
| Carrier slot voltage | Nominal 24 V | Direct high-current backplane feed. |
| Carrier slot input target | Approximately 4.7 A | 100 W output at 90% efficiency is approximately 4.63 A input. |
| Rev A carrier slots | 6 | Firmware/protocol logical capacity remains 8. |
| USB-C PD policy ceiling | 100 W | Maximum 20 V, 5 A; no EPR or proprietary 7 A mode. |
| Backpack logic rail | 3.3 V | Local LMR51610 supplies MCU, mux, CAN PHY, and status LED. |
| Backpack fan rail | 12 V | Local LMR51610; final fan start/stall budget remains open. |

## Protection Checklist

- Input fuse or breaker.
- Per-slot fuse, eFuse, current limit, or zero-ohm bring-up option.
- Reverse polarity protection, if input connector allows it.
- TVS or surge strategy for the DC input.
- I2C pull-up placement and voltage domain.

## USB-C PD Modules

It turns out that the [SW3538 modules](https://github.com/happyme531/h1_SW35xx/issues/13#issuecomment-4361605621) use a non-standard configuration to reach their advertised "140W" spec; proprietary 20 V at 7 A mode.
Standard USB PD 3.0 tops out at 20 V at 5 A (100 W) which is already _plenty_ for my intended loads (they top out at around 70W!).

Firmware enforces 100 W as the maximum accepted/programmed port policy. Rev A has
no MCU-controlled high-side switch, so this is a soft ceiling after SW3538
initialization rather than an independent protection device. Provision the backpack,
install modules, persist policy, and only then attach loads.

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

For the simplified backplane, assume a 30 A maximum shared input current, 2 oz external copper (nominally 70 µm or 2.756 mil), and a 10 °C conductor temperature rise.
The IPC-2221 relationship used by KiCad is:

`I = k × ΔT^0.44 × (W × H)^0.725`

For an external conductor, `k = 0.048`.
Solving for width at 30 A gives approximately 644.5 mil, or 16.37 mm, on one external layer.
KiCad's own [formula notes](https://gitlab.com/kicad/code/kicad/-/blob/10.0/pcb_calculator/tracks_width_versus_current_formula.md) state that the model is valid only up to a 400 mil (10 mm) width, so that single-layer result is an extrapolation and should be treated as a rough engineering estimate.

The prototype therefore uses matching F.Cu and B.Cu pours in parallel. An ideal
50/50 split at 30 A is 15 A per layer, which requires 247.8 mil or 6.29 mm per
layer. The stated 28.5 A maximum sustained load requires approximately 5.86 mm
per layer under the same assumptions.

The airflow slots leave 8.0 mm-wide board webs at the top and bottom. After the
0.5 mm copper-to-edge clearance and zone-fill rounding, the narrowest filled
copper section is 6.9 mm on each outer layer. The formula estimates 16.04 A per
6.9 mm conductor, or 32.1 A nominal combined capacity before accounting for
unequal current sharing, connector-pad transitions, and real thermal
conditions. The pours connect all input and XT30 power pads solidly on both
layers; any future non-pad layer transition needs a separately engineered via
array.

Each 4.7 A slot branch requires approximately 1.27 mm on one 2 oz external
layer. Solid zone connections provide substantially more copper than that at
each XT30 pad. The paired rails must remain on both outer layers.
