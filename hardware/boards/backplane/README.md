# Backplane PCB

KiCad project for the mini-rack power backplane.

Responsibilities:

- Host carrier board mating connectors.
- Distribute DC power to each carrier slot.
- Provide an I2C mux, expected to be TCA9548A-family, to isolate fixed-address carrier modules.
- Carry the slot-status LED chain.
- Define slot numbering and slot-local net naming.

The controller/I2C-mux/LED arrangement represented by this board revision predates
the active Backplane Backpack architecture. Preserve the source as hardware history,
but reconcile it with the backpack interface before using it as a manufacturing
source. The active controller, mux, fan, and status LED now live on the
`backplane-backpack` board.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
