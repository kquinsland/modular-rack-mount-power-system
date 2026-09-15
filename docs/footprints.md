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

## DEGSON DG135T-10.16-02P

Footprint: `hardware/libraries/footprints/mini-rack-power.pretty/DEGSON_DG135T-10.16-02P.kicad_mod`

Selected orderable part: DEGSON `DG135T-10.16-02P-14-00A(H)`, LCSC
`C708738`.

Sources:

- [DEGSON product page](https://www.degson.com/content/details_552_880568.html?lang=en)
- DEGSON customer drawing `200031772`, revision 01.
- [LCSC C708738 listing](https://www.lcsc.com/product-detail/C708738.html)

The manufacturer drawing specifies:

- Two poles with two internally common through-hole pins per pole.
- 10.16 mm pole pitch and 10.16 mm spacing between the two pin rows.
- 2.40 mm finished PCB holes for 1.60 mm x 1.20 mm terminals.
- A 20.32 mm x 18.70 mm nominal body envelope for the two-pole variant.
- A 29.50 mm nominal height above the PCB.

The footprint uses 5.00 mm copper pads around the specified 2.40 mm holes. Pads
in each front/rear pair intentionally share the same pad number so the generic
two-pin schematic symbol maps one electrical pin to each screw terminal. The
fabrication layer identifies the wire-entry side. The body position relative to
the pin grid was cross-checked against LCSC's EasyEDA model for `C708738`.

## Adjustable DC-to-DC Module

Symbol: `hardware/libraries/symbols/mini-rack-power.kicad_sym`, symbol
`Adjustable_DC2DC`.

Footprint: `hardware/libraries/footprints/mini-rack-power.pretty/Adjustable_DC2DC.kicad_mod`

3D model: `hardware/libraries/3dmodels/Adjustable_DC2DC.step`

The symbol, footprint, and STEP model were copied from
`/mnt/sync/Projects/dog-water-bowl/eCAD`. The four through-hole pins are on a
2.54 mm pitch and map as 1=EN, 2=V_IN, 3=GND, and 4=V_OUT. The copied STEP file
has SHA-256
`e6c9628a6866ffa3286b3e2aab7c6efffb05d5e2a6110392d9e063b4814d6716`.

The local footprint adds a full module-body fabrication outline and courtyard.
On the backplane, `PS1` takes `+24V` at V_IN and exposes its uncommitted output
as `ADJ_DC_OUT`; the output is intentionally not tied to an existing rail until
the module setpoint and destination are selected.
