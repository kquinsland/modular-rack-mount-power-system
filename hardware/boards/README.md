# Boards

Each physical PCB gets its own KiCad project directory.

Current boards:

- `backplane`
- `backplane-backpack`
- `backplane-prototype`
- `carrier`

Historical/superseded board sources retained in the repository:

- `controller` (WT32 architecture, superseded by `backplane-backpack`)

Future boards should follow the same pattern: `hardware/boards/<board-name>/<board-name>.kicad_pro`.
