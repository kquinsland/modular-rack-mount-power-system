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

Footprint: `hardware/libraries/footprints/mini-rack-power.pretty/Amass_XT30U_2plus2_F.kicad_mod`

Source: `docs/data-sheets/Amass/XT30U(2+2)-F.G.B.pdf`

Mechanical sources:

- `hardware/mechanical/exports/XT30(2+2).dxf`
- `hardware/mechanical/exports/XT30(2+2).step`

The footprint is for the vertical / 180-degree female connector and uses the imported mating-face geometry:

- Power contacts: plated through-hole pads centered on the two circular metal-pin features.
- Signal contacts: front-side SMT pads centered on the two rectangular solder-foot features.
- Pad numbering follows the backplane schematic assumption: 1=DC_IN+, 2=GND, 3=SDA, 4=SCL.
