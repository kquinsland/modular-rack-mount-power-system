# Footprint Notes

## Amass XT30PW(2+2)-M.G.B

Footprint: `hardware/libraries/footprints/mini-rack-power.pretty/Amass_XT30PW_2plus2_M_RightAngle_THT.kicad_mod`

Source: `docs/data-sheets/Amass/XT30PW(2+2)-M.G.B.pdf`

The through-hole footprint uses the manufacturer installation drawing:

- Power pins: 2 holes, 1.90 mm drill.
- Power pin pitch: 5.00 mm.
- Signal pins: 0.60 mm x 0.60 mm square terminals, represented as 0.90 mm round drills.
- Signal pin pitch: 2.00 mm.
- Signal pin column offset from the nearest power pin: 3.85 mm.

The signal pads use 1.60 mm copper with 0.90 mm drill. The power pads use 3.20 mm copper with 1.90 mm drill.

## Amass XT30U(2+2)-F.G.B

Footprint: `hardware/libraries/footprints/mini-rack-power.pretty/Amass_XT30U_2plus2_F_RightAngle_Mechanical.kicad_mod`

Source: `docs/data-sheets/Amass/XT30U(2+2)-F.G.B.pdf`

The available datasheet gives the connector envelope but does not provide a recommended PCB land pattern or exposed solder land dimensions. The project footprint is therefore mechanical-only:

- No electrical pads.
- Excluded from BOM and position files.
- Courtyard and fab/silk outlines are based on the 12.70 mm by 13.30 mm envelope shown in the datasheet.

Replace this with a real SMT land-pattern footprint before routing or manufacturing a board that uses this part electrically.
