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
The breakout header for each slot is directly above its XT30 connector and uses
this pinout:

1. GND
2. SDA
3. SCL

The rectangular board outline is 165 mm by 50 mm. The PCB is intentionally
unrouted: its purpose is CAD and physical-fit verification, not powered use.
ESD protection, LEDs, control electronics, and the remaining production
backplane circuitry are deliberately omitted.
