# Carrier + backplane Rev A fabrication panel

This customer panel contains six released carrier PCBs and one released
backplane-prototype PCB. It measures 163.800 x
237.350 mm and is intended for top-side assembly.
The top rail is marked `PANEL 35e47c0` on F.SilkS.

- `jlcpcb/`: JLCPCB upload bundle, separate files, and order notes.
- `pcbway/`: PCBWay upload bundle, separate files, and order notes.
- `modular-rack-power-carrier6-backplane1-rev-a.kicad_pcb`: generated panel source for CAM review.
- `modular-rack-power-carrier6-backplane1-rev-a-top.webp`: top-side WebP render for visual review.
- `panel-info.json`: dimensions, construction, source revisions, and counts.
- `validation.json`: generated DRC and Gerber archive checks.

Each vendor bundle contains one nested Gerber ZIP, `BOM.csv`, `positions.csv`,
and `ORDER-NOTES.txt`. Read the vendor order notes before placing an order.

The panel is reproducible with:

```sh
mise run panel:build
```
