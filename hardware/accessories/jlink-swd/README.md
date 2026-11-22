# J-Link SWD adapter

This standalone 56 × 42 mm, two-layer board plugs directly into the SEGGER 20-pin ARM debug port. The keyed six-pin cable reaches backplane J10 or carrier J2. Both targets use the same contact assignments.

| Ref | Fit | Assembly |
| --- | --- | --- |
| J1 | Sullins SFH11-PBPC-D10-RA-BK ([LCSC C3319874](https://www.lcsc.com/product-detail/C3319874.html)) | Polarized 2×10 right-angle **female** socket, component side. Its mouth projects 7.74 mm past the PCB edge. |
| J2 | Leave empty | Central 2×10 through-hole tap. Every position connects to the same numbered position on J1. |
| J3 | 2×3 keyed vertical IDC **male** box header, 2.54 mm | Match the shroud opening to the footprint outline. Plug the TC2030-IDC-NL ribbon socket in here. |
| J4 | 1×3 straight male pin header, 2.54 mm | Pin 1 is target 3V3, pin 2 is GND, and pin 3 is J-Link 5V. |
| J5 | External 3.3 V input, 1×2 | Pin 1 is regulated 3.3 V, pin 2 is GND. |
| J6 | Source selector, 1×3 | Fit one shunt. Pins 1–2 select the probe-powered regulator, pins 2–3 select J5. |
| J7 | Target-current link, 1×2 | Pin 1 is the selected source, pin 2 is the target rail. Fit a shunt for normal adapter-powered operation. |
| J8 | Ground header, 1×3 | All three contacts are GND. |
| J9 | VTref sense link, 1×2 | Pin 1 goes to J-Link VTref, pin 2 to the target. Fit for a probe configured to sense the target voltage. |
| H1–H4 | 3.0 mm plated GND holes with 6.0 mm exposed pads | Hole centers form a 26.8 × 35.0 mm rectangle. Each pad connects to the top and bottom GND pours. Conductive mounting hardware touching a pad shares the target and J-Link ground. |

## Probe connections

| J3 / target pad | Signal | J1 / J2 |
| ---: | --- | --- |
| 1 | Target 3V3 | 1 through J9 |
| 2 | SWDIO | 7 |
| 3 | Active-low reset | 15 |
| 4 | SWCLK | 9 |
| 5 | GND | 4, 6, 8, 10, 12 |
| 6 | Open | — |

## Power and current profiling

Change shunts with the probe and target unpowered. Pin 1 of each new header has a square pad. J4 keeps the v1.0 tap order. Its 3V3 contact is on the target side of the current link. Its 5V contact connects directly to J-Link pin 19.

| Target power | J6 | J7 | J9 |
| --- | --- | --- | --- |
| Target's own supply | Open | Open | Fit for VTref sensing |
| J-Link pin 19 via the onboard 3.3 V regulator | 1–2 | Fit | Fit for VTref sensing |
| External regulated 3.3 V at J5 | 2–3 | Fit | Fit for VTref sensing |
| Current profiling on either adapter source | Select source | Replace shunt with ammeter | Fit only for a sense-only VTref input |

For probe-powered operation, enable the probe's pin-19 5 V output if the probe supports it. U1 (MCP1700-3302E/TO) supplies the target through J6 and J7. At 5 V input and 100 mA load it dissipates about 0.17 W. Check the probe output rating and regulator thermal limits for the intended load. For external operation, connect the supply positive to J5.1 and its return to J5.2. Disconnect the target's regular power input before using either adapter source.

For a series current measurement, connect the meter's positive current input to J7.1 and its return to J7.2. For a power profiler with its own supply, feed J5 from the profiler, select EXT, and fit J7. The regulator and its input current are upstream of the measurement. Other powered connections to the target can contribute current outside this path. If the probe actively drives VTref, disable that output before fitting J9. Opening J9 isolates pin 1 for a measurement, though a probe that requires VTref may then refuse to debug. The grounds stay common in every mode.

Reserved J-Link contacts 14, 16, 18, and 20 each have their own tap.

J2’s front silkscreen follows the SEGGER pin numbers. Odd positions are on the left and even positions on the right when the text is upright. R denotes a reserved position. NC marks pins 2 and 17 in standard SWD mode.

## Mating orientation

With J1's polarization bump up, looking into the **female mating face**, the upper row runs from SEGGER pin 1 at the left to pin 19 at the right. The lower row runs from pin 2 to pin 20. The upper contacts bend to the through-hole row farther from the board edge. The square J1 pad is VTref. Before powering a first assembly, verify continuity from that socket contact through the fitted J9 link to J3 pin 1 and from the opposite end of the same row to J4 pin 3.

See the [SEGGER 20-pin connector drawing](https://kb.segger.com/20-pin_J-Link_Connector), the [Sullins SFH11 mechanical drawing](https://sullinscorp.com/catalogs/147_PAGE119_.100_SFH11_SERIES_FEMALE_HDR_ST_RA.pdf), and the [TC2030-IDC-NL cable pin map, sheet 2](https://www.tag-connect.com/wp-content/uploads/bsk-pdf-manager/2019/12/TC2030-IDC-NL-Datasheet-Rev-B.pdf#page=2).

The project includes a [mechanical STEP model of J1](models/SFH11-PBPC-D10-RA-BK.step) with a keyed housing, mating openings, and 20 solder tails. It was drawn from the Sullins dimensions and the assembly outline for this part in the [TI reference design](https://www.ti.com/lit/zip/SWRC397). Internal spring contacts are omitted.

## Production export

The front silkscreen identifies revision 1.1. The KiCad editor displays `YY-MM-DD` and `UNRELEASED` as working-copy placeholders, as on the main boards. After committing the layout being fabricated, generate the fabrication files and purchasing BOM with KiCad 10 and the JLCPCB Fabrication Toolkit installed:

```sh
mise run production:jlink-swd
```

The default output directory is `hardware/accessories/jlink-swd/production/YY.MM.DD-<hash>/`. The directory name and silkscreen both use the commit’s UTC date and first 12 hex digits. To choose a different destination, run `mise run production:jlink-swd /path/to/output`.

Select bare-board fabrication and upload only `jlink-swd.zip`. All connector pads and drills are included for hand soldering. The separate `positions.csv` contains its column headings and zero placement rows. The export also writes the footprint inventory (`designators.csv`) and electrical-test netlist (`netlist.ipc`).

Use `bom.csv` for the hand-assembled components. Leave J2 empty and obtain three removable 2.54 mm jumper shunts for J6, J7, and J9.

The task uses `/usr/bin/python3` by default. Set `KICAD_PYTHON` to another interpreter with the KiCad and wxPython bindings if needed.
