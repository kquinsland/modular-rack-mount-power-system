# Boards

Each physical PCB gets its own KiCad project directory.

Current boards:

- `backplane`
- `carrier`

`backplane` is the consolidated project formerly called `backplane-prototype`.
The earlier split `backplane`, `backplane-backpack`, and WT32 `controller`
projects have been retired; their sources remain available in Git history.
Shared CAN/protection symbols were preserved in
`hardware/libraries/symbols/backplane-backpack.kicad_sym`. That library nickname
is retained for compatibility and does not imply a separate active board.

Future boards should follow the same pattern: `hardware/boards/<board-name>/<board-name>.kicad_pro`.
