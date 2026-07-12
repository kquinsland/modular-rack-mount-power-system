# Backplane prototype

This KiCad project is a stripped-down mechanical and dimensional prototype of
the production backplane. It is intended to verify connector spacing, panel
clearance, and mounting-hole placement before the complete backplane is built.

The PCB intentionally contains only:

- six combined XT30 and two-signal slot connectors (`J2`–`J7`);
- six 1x3, 2.54 mm-pitch breakout headers (`J8`–`J13`);
- the main 24 V screw-terminal input (`J1`); and
- four M3 mounting holes (`H1`–`H4`).

The XT30 connectors are vertical, with their two low-voltage contacts at the
top, and are arranged in one horizontal row at 19 mm center-to-center spacing.
The bottom-side breakout header for each slot is directly above its XT30
connector. Each header has matching `GND`, `SDA`, and `SCL` bottom-silkscreen
labels and uses this pinout:

1. GND
2. SDA
3. SCL

The tightened rectangular board outline is 143.8 mm by 38.5 mm. The mounting
holes form aligned columns at x = 33.3 mm and x = 169.5 mm, with rows at
y = 48.0 mm and y = 75.9 mm.

Five vertical airflow cutouts are centered between adjacent XT30 connectors.
Each cutout is an 8.0 mm-wide obround extending from y = 49.2 mm to
y = 71.7 mm, leaving 8.0 mm of material to both the top and bottom board
edges.

SDA and SCL are routed on F.Cu from each bottom-side header to its matching
XT30 signal contacts. The two outer layers remain specified as 2 oz copper,
but the `+24V` and `GND` pours are temporarily removed while the high-current
routing is revised. The power pads, header GND pins, and grounded mounting
holes therefore remain intentionally unrouted. ESD protection, LEDs, control
electronics, and the remaining production backplane circuitry are deliberately
omitted.
