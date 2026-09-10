# Power Budget

Track the system and per-slot power assumptions here.

| Item | Working value | Notes |
| --- | ---: | --- |
| Backplane/system nominal input | 24–48 V | Designed for both carrier generations; final supply tolerance and first-article qualification remain open. |
| Current carrier input envelope | 24 V nominal; 30 V maximum | The populated SW3538 carrier limits the first revision. A future carrier revision is required for 48 V operation. |
| Backplane sustained load target | Approximately 30 A | Six ports near 4.7 A input plus system margin. |
| Carrier slot voltage | Same as backplane input | Direct, unswitched feed; use 24 V nominal and never exceed 30 V while any current-generation carrier is fitted. |
| Carrier slot input target | Approximately 4.7 A | 100 W output at 90% efficiency is approximately 4.63 A input. |
| Carrier slots | 6 | One unswitched high-current feed per physical slot. |
| USB-C PD policy ceiling | 100 W | Maximum 20 V, 5 A; no EPR or proprietary 7 A mode. |
| Backplane logic rail | 3.3 V | Local LM5164DDAR/C477928 supplies the MCU, current sensor, CAN PHY, and externally powered DS18B20 header. |
| Backplane fan rail | 12 V | Separate 1 A LM5164DDAR/C477928 feeding two independently switched 3-wire fan headers; combined continuous/startup/stall loading requires first-article testing. |
| Aggregate current shunt | 1 mOhm | 30 mV and 0.9 W at 30 A; INA237 narrow-range configuration and Kelvin routing required. |

Both backplane converters use PSPMAA0805-101M-ANP, LCSC/JLCPCB C2962892,
100 uH as their common inductor. Their feedback, RON, ripple-injection, and
capacitor values remain rail-specific.

The carrier housekeeping converter now uses the same LM5164DDAR/C477928 and
C2962892 pair. At the first JLCPCB quote, confirm whether C2962892 or either
completed board requires an assembly fixture. If a fixture is required, revisit
the common inductor selection across both boards before production release.

## Protection Checklist

- System-level upstream current monitoring, fuse, breaker, or equivalent fault
  interrupter for the raw DC feed.
- Per-slot fuse, eFuse, current limit, or zero-ohm bring-up option.
- Reverse-polarity prevention/protection. For the carrier, the keyed XT
  connector is the intended normal-use prevention mechanism; add electrical
  protection if any adapter, service lead, or alternate harness can defeat it.
- TVS or surge strategy for the DC input.
- I2C pull-up placement and voltage domain.

## USB-C PD Modules

It turns out that the [SW3538 modules](https://github.com/happyme531/h1_SW35xx/issues/13#issuecomment-4361605621) use a non-standard configuration to reach their advertised "140W" spec; proprietary 20 V at 7 A mode.
Standard USB PD 3.0 tops out at 20 V at 5 A (100 W) which is already _plenty_ for my intended loads (they top out at around 70W!).

The carrier's MCU-controlled hot-swap can disconnect the PD branch, but it is
not an independent 100 W limiter. The 100 W value is a system operating target,
not a backplane hardware current limit.

The backplane is intended for a 24–48 V nominal system. The current SW3538
carrier is the limiting population: 24 V nominal input, 30 V maximum input,
and 20 V / 5 A-output, 100 W operating policy. A second-generation carrier
with a suitable module and coordinated protection is required for 48 V input;
testing alone cannot qualify the existing SW3538 population for that voltage.
The later 240 W USB-C EPR goal also requires a fresh power-path, TVS, OVLO,
SOA, transient, and thermal review. The backplane does not convert or limit
slot voltage, so a 48 V source must never be used with a current carrier fitted.

There is no below-24 V nominal system requirement. Low-line tests account for
supply tolerance and harness drop; they are not additional nominal ratings.
The carrier UVLO turn-on corner can reach approximately 22.56 V at 25 °C,
so specify the minimum voltage at the carrier and validate cold/hot startup.

The consolidated backplane does not retain the old PCA9554-controlled per-slot
FETs. All six slot feeds are present whenever `VIN_BUS` is energized, so upstream
and/or per-slot hardware fault protection must be resolved independently of
firmware.

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
