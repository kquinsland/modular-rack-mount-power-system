# Backplane prototype

This KiCad project is a stripped-down mechanical and dimensional prototype of
the production backplane. It is intended to verify connector spacing, panel
clearance, and mounting-hole placement before the complete backplane is built.

The PCB intentionally contains only:

- six combined XT30 and two-signal slot connectors (`J2`–`J7`);
- six 1x3, 2.54 mm-pitch breakout headers (`J8`–`J13`);
- the main 24 V screw-terminal input (`J1`); and
- three M3 mounting holes (`H1`, `H2`, and `H4`).

The XT30 connectors are vertical, with their two low-voltage contacts at the
top, and are arranged in one horizontal row at 19 mm center-to-center spacing.
The bottom-side breakout header for each slot is oriented horizontally and
sits directly above its XT30 connector. Each header has matching `GND`, `SDA`,
and `SCL` bottom-silkscreen labels and uses this pinout:

1. GND
2. SDA
3. SCL

The rectangular board outline is 151.8 mm by 30.8 mm. Every XT30 connector is
centered at y = 62.1 mm, exactly on the board's horizontal centerline. The left
mounting hole is centered beside the screw terminal at x = 54.214 mm and
y = 61.962 mm. The right mounting holes are at x = 177.5 mm and y = 51.512 mm
and y = 73.7 mm, clear of the final horizontal breakout header.

Five vertical airflow cutouts are centered between adjacent XT30 connectors.
Each cutout is an 8.0 mm-wide by 15.5 mm-tall obround extending from
y = 54.35 mm to y = 69.85 mm, leaving 7.65 mm of material to both the top and
bottom board edges.

SDA and SCL are routed on F.Cu from each bottom-side header to its matching
XT30 signal contacts. The `+24V` and `GND` paths use matching solid pours on
F.Cu and B.Cu for the 30 A input budget, with both outer layers specified as
2 oz copper. The airflow-slot clearances leave a 6.55 mm minimum filled-copper
neck on each layer, with an IPC-2221 estimate of approximately 30.9 A combined
at a 10 °C rise. Every power-pad connection is solid rather than thermal-relief
connected. The lower-right grounded mounting hole remains intentionally
unrouted. ESD protection, LEDs, control electronics, and the remaining
production backplane circuitry are deliberately omitted.
