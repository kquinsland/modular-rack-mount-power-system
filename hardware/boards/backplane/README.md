# Backplane PCB

KiCad project for the mini-rack power backplane. The board is being reconciled
with `backplane-prototype` and `backplane-backpack`; the prototype dimensions,
connector footprints, and placement are the mechanical authority, while the
carrier is the electrical authority where the designs overlap.

## Reconciliation decisions

- Use CAN-FD between the backplane and every carrier. Do not carry forward the
  former per-slot I2C mux or its ESD parts. Backplane I2C is local only between
  the STM32 and the board current sensor.
- Use the carrier's STM32 and TCAN3413 CAN-FD transceiver choices.
- Generate both `+3V3` and the 3-wire fan's `+12V` rail with separate
  **LM5164DDAR** converters, LCSC/JLCPCB **C477928**.
- Use **PSPMAA0805-101M-ANP**, 100 uH, LCSC/JLCPCB **C2962892**, as the common
  inductor for both backplane converters. Each rail still requires its own
  feedback, RON, ripple-injection, and capacitor calculations.

### Assembly quote checkpoint

During the first JLCPCB assembly quote, explicitly confirm whether C2962892 or
the resulting board construction requires a PCB assembly fixture. The live
C2962892 part page does not currently show a fixture warning, so no fixture
requirement is assumed at design time.

- If no fixture is required, retain C2962892 and subsequently update the
  carrier housekeeping converter to LM5164DDAR/C477928 with C2962892 so the
  regulator and inductor setup fees are consolidated across boards.
- If a fixture is required, revisit the common inductor choice before changing
  the carrier.

This is a quote-stage verification item, not a reason to block the current
backplane design.

## Historical project responsibilities

Responsibilities:

- Host carrier board mating connectors.
- Distribute DC power to each carrier slot.
- Provide an I2C mux, expected to be TCA9548A-family, to isolate fixed-address carrier modules.
- Carry the slot-status LED chain.
- Define slot numbering and slot-local net naming.

The controller/I2C-mux/LED arrangement represented by this board revision is
historical and must not be used as a manufacturing source. The reconciliation
work supersedes the earlier split backplane/backpack architecture.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
