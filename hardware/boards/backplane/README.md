# Backplane PCB

KiCad project for the mini-rack power backplane.

Responsibilities:

- Host carrier board mating connectors.
- Distribute DC power to each carrier slot.
- Provide an I2C mux, expected to be TCA9548A-family, to isolate fixed-address carrier modules.
- Define slot numbering and slot-local net naming.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
