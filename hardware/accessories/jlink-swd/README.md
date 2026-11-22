# J-Link SWD adapter

This standalone 51.2 × 42 mm, two-layer board plugs directly into the SEGGER 20-pin ARM debug port. The keyed six-pin cable reaches backplane J10 or carrier J2. Both targets use the same contact assignments.

| Ref | Fit | Assembly |
| --- | --- | --- |
| J1 | Sullins SFH11-PBPC-D10-RA-BK ([LCSC C3319874](https://www.lcsc.com/product-detail/C3319874.html)) | Polarized 2×10 right-angle **female** socket, component side. Its mouth projects 7.74 mm past the PCB edge. |
| J2 | Leave empty | Central 2×10 through-hole tap. Every position connects to the same numbered position on J1. |
| J3 | 2×3 keyed vertical IDC **male** box header, 2.54 mm | Match the shroud opening to the footprint outline. Plug the TC2030-IDC-NL ribbon socket in here. |
| J4 | 2×2 straight male pin header, 2.54 mm | Pins 1 and 3 tap J-Link pin 19 (5 V where supported). Pins 2 and 4 are GND. |
| J5 | External 3.3 V input, 1×2 | Pin 1 is regulated 3.3 V, pin 2 is GND. |
| J6 | Source selector, 1×3 | Fit one shunt. Pins 1–2 select J-Link pin 1 (VTref), pins 2–3 select J5. |
| J7 | Target-current link, 1×2 | Pin 1 is the selected source, pin 2 is the target rail. Fit a shunt for normal adapter-powered operation. |
| J10 | Clone 3.3 V auxiliary tap, 2×2 | Pins 1 and 3 tap J-Link pin 2. Pins 2 and 4 are GND. |
| H1–H4 | 3.0 mm plated GND holes with 6.0 mm exposed pads | Hole centers form a 40.8 × 35.0 mm rectangle. Each pad connects to the top and bottom GND pours. Conductive mounting hardware touching a pad shares the target and J-Link ground. |

## Probe connections

| J3 / target pad | Signal | J1 / J2 |
| ---: | --- | --- |
| 1 | Target 3V3 | 1 through J6 (1–2) and J7 |
| 2 | SWDIO | 7 |
| 3 | Active-low reset | 15 |
| 4 | SWCLK | 9 |
| 5 | GND | 4, 6, 8, 10, 12 |
| 6 | Open | — |

## Power and current profiling

Change shunts with the probe and target unpowered. J4 and J10 each provide two power/GND pairs for leads. A shunt across either pair would short the supply. The odd-numbered column carries power and the even-numbered column is ground. Pin 1 has a square pad.

| Mode | J6 | J7 | Connection |
| --- | --- | --- | --- |
| Self-powered target with standard VTref sensing | 1–2 | Fit | Target 3.3 V reaches the probe’s pin 1 so it can sense the target logic level. |
| Probe with a verified supply on **pin 1** | 1–2 | Fit | Pin 1 can feed an otherwise unpowered target only when that probe explicitly supports it. |
| External regulated 3.3 V | 2–3 | Fit | Connect supply positive to J5.1 and return to J5.2. Probe pin 1 is isolated. |
| Measure current supplied by the adapter | Select the active supply | Replace shunt with ammeter | Meter positive to J7.1 (source), return to J7.2 (target). |

SEGGER specifies pin 1 as a target-voltage reference input. Selecting VTref with a standard probe and an unpowered target supplies no power. Conversely, external mode disconnects the probe’s VTref input, so a debugger that requires target-voltage sensing may refuse to attach in that mode. To debug an independently powered target using standard voltage sensing, use the first row.

The clone’s pin-2 output is available at J10 independently of J6. Verify the voltage and current capability of the particular probe before drawing power. To use that output as the target supply, patch J10.1 or J10.3 to J5.1, select EXT and fit J7. For a profiler with its own supply, feed J5 directly from the profiler. When the adapter powers the target, disconnect its normal power input. Grounds remain common throughout.

Reserved J-Link contacts 14, 16, 18, and 20 each have their own tap.

J2’s front silkscreen follows the SEGGER pin numbers. Odd positions are on the left and even positions on the right when the text is upright. R denotes a reserved position. P2 marks the optional clone supply and NC marks pin 17.

## Mating orientation

With J1's polarization bump up, looking into the **female mating face**, the upper row runs from SEGGER pin 1 at the left to pin 19 at the right. The lower row runs from pin 2 to pin 20. The upper contacts bend to the through-hole row farther from the board edge. The square J1 pad is VTref. Before powering a first assembly, verify continuity from that socket contact through J6 (1–2) and the fitted J7 link to J3 pin 1 and from the opposite end of the same row to both J4 pins 1 and 3.

See the [SEGGER 20-pin connector drawing](https://kb.segger.com/20-pin_J-Link_Connector), the [Sullins SFH11 mechanical drawing](https://sullinscorp.com/catalogs/147_PAGE119_.100_SFH11_SERIES_FEMALE_HDR_ST_RA.pdf), and the [TC2030-IDC-NL cable pin map, sheet 2](https://www.tag-connect.com/wp-content/uploads/bsk-pdf-manager/2019/12/TC2030-IDC-NL-Datasheet-Rev-B.pdf#page=2).

The project includes a [mechanical STEP model of J1](models/SFH11-PBPC-D10-RA-BK.step) with a keyed housing, mating openings, and 20 solder tails. It was drawn from the Sullins dimensions and the assembly outline for this part in the [TI reference design](https://www.ti.com/lit/zip/SWRC397). Internal spring contacts are omitted.

## Production export

The front silkscreen identifies revision 1.1. The KiCad editor displays `YY-MM-DD` and `UNRELEASED` as working-copy placeholders, as on the main boards. After committing the layout being fabricated, generate the fabrication files and purchasing BOM with KiCad 10 and the JLCPCB Fabrication Toolkit installed:

```sh
mise run production:jlink-swd
```

The default output directory is `hardware/accessories/jlink-swd/production/YY.MM.DD-<hash>/`. The directory name and silkscreen both use the commit’s UTC date and first 12 hex digits. To choose a different destination, run `mise run production:jlink-swd /path/to/output`.

Select bare-board fabrication and upload only `jlink-swd.zip`. All connector pads and drills are included for hand soldering. The separate `positions.csv` contains its column headings and zero placement rows. The export also writes the footprint inventory (`designators.csv`) and electrical-test netlist (`netlist.ipc`).

Use `bom.csv` for the hand-assembled components. Leave J2 empty and obtain two removable 2.54 mm jumper shunts for J6 and J7.

The task uses `/usr/bin/python3` by default. Set `KICAD_PYTHON` to another interpreter with the KiCad and wxPython bindings if needed.
