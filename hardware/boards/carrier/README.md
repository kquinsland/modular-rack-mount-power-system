# Carrier PCB

KiCad project for one mini-rack power carrier board.

Responsibilities:

- Mate with one backplane slot.
- Host one fixed-address DC/USB-C PD module.
- Convert the slot-local DC and I2C interface into the module-local wiring.
- Keep carrier-side interfaces reusable across all backplane slots.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
