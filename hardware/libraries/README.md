# KiCad Libraries

Project-local libraries shared by all board projects.

- `symbols/mini-rack-power.kicad_sym`: Custom schematic symbols.
- `symbols/backplane-backpack.kicad_sym`: Shared CAN/protection symbols relocated
  from the retired backpack project. The historical nickname is retained by
  both active projects to preserve symbol identities.
- `footprints/mini-rack-power.pretty/`: Custom footprints.
- `3dmodels/`: STEP/WRL models referenced by project footprints.

Keep third-party vendor libraries out of this directory unless they are vendored intentionally for reproducible builds.

The unused WT32-ETH01 submodule and library-table registrations were removed
with the retired controller project.
