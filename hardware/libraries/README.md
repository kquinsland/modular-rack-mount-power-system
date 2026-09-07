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
