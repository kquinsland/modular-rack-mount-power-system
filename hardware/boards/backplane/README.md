# Legacy backplane project

This directory contains the superseded split-backplane design. It remains in
the repository as design history and is not a manufacturing source.

The active reconciliation is implemented in
`../backplane-prototype/backplane-prototype.kicad_sch`. That project uses the
prototype PCB as the mechanical authority and the carrier as the electrical
authority where the boards overlap.

The active design decisions are:

- one combined backplane/controller PCB with six carrier slots;
- CAN-FD to every carrier, with I2C local only between the backplane STM32 and
  INA237 current sensor;
- the carrier's STM32C092GCU6, TCAN3413, INA237, CAN-protection parts, slot
  connector/pinout, and WS2812B-2020-V6 LED choice;
- one protected external GND/CANL/CANH screw-terminal connection and optional
  120 ohm termination, with no duplicate backplane ESD network per slot;
- separate LM5164DDAR/C477928 converters for 3.3 V and the 3-wire fan's 12 V
  rail; and
- PSPMAA0805-101M-ANP/C2962892 100 uH as the common regulator inductor.

## Assembly quote checkpoint

During the first JLCPCB assembly quote, explicitly confirm whether C2962892 or
the resulting board construction requires a PCB assembly fixture. Continue
with the selected part for design and quoting; revisit it only if the quote
actually requires a fixture.

If no fixture is required, the carrier housekeeping converter may subsequently
be migrated to the same LM5164DDAR/C477928 and C2962892 pair to consolidate
regulator and inductor setup fees across boards.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
