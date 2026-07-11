# Backplane PCB

KiCad project for the mini-rack power backplane.

Responsibilities:

- Host carrier board mating connectors.
- Distribute DC power to each carrier slot.
- Provide an I2C mux, expected to be TCA9548A-family, to isolate fixed-address carrier modules.
- Carry the slot-status LED chain.
- Define slot numbering and slot-local net naming.

The ESP32 and fan circuit are intentionally located on the separate `controller`
board. The upstream I2C header carries 3.3 V logic power; the separate LED header
carries 5 V.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
