# Boards

Each physical PCB gets its own KiCad project directory.

Current boards:

- `backplane`
- `carrier`

Both active boards use the same STM32C092GCU6 and direct CAN-FD architecture.
The backplane and carrier directories are the only product board definitions;
earlier research layouts are not compatibility targets. Some shared library
nicknames retain their original internal name so the finalized KiCad references
remain stable.

Future boards should follow the same pattern: `hardware/boards/<board-name>/<board-name>.kicad_pro`.
