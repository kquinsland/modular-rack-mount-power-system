# KiCad Libraries

Project-local libraries shared by all board projects.

- `symbols/mini-rack-power.kicad_sym`: Custom schematic symbols.
- `symbols/backplane-backpack.kicad_sym`: Shared CAN/protection symbols relocated
  from the retired backpack project. The historical nickname is retained by
  both active projects to preserve symbol identities.
- `footprints/mini-rack-power.pretty/`: Custom footprints.
- `3dmodels/`: STEP/WRL models referenced by project footprints.

`mini-rack-power:D_SMC_Silk0.15mm` is a project-local derivative of KiCad's
`Diode_SMD:D_SMC` (DO-214AB). Its three silkscreen lines use 0.15 mm strokes
instead of 0.12 mm to satisfy the project's legend rule. The copper pad
geometry, fabrication outline, courtyard, and stock 3D-model reference are
preserved. Carrier D1 uses this version so a library refresh retains the fix.

Keep third-party vendor libraries out of this directory unless they are vendored intentionally for reproducible builds.

The unused WT32-ETH01 submodule and library-table registrations were removed
with the retired controller project.

`mini-rack-power:PCBWay_Logo_25mm` is the supplied PCBWay SVG converted to
filled front-silkscreen vector polygons, preserving the underline and letter
openings. Visible artwork is 25.000 mm wide and approximately 7.007 mm high;
the footprint origin is its center. It has no pads and is board-only, excluded
from BOM and position exports. The source is `PCBWay Logo PNG SVG Vector.svg`
at the repository root. Copies and separate `WayWayWay` text are staged above
the top-center of each individual board for later placement. The first
prototype PCBWay submission is per project, not a combined panel.

`mini-rack-power:PCBWay_Logo_15mm` is the same artwork scaled uniformly to
15.000 × 4.204 mm. Both sizes remain available; the carrier and backplane
now use the 15 mm version, preserving the placed centers, orientations and
sides. The separate `WayWayWay` text is unchanged.
