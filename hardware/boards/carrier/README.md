# Carrier PCB

KiCad project for one mini-rack power carrier board.

Responsibilities:

- Mate with one backplane slot.
- Host one fixed-address DC/USB-C PD module.
- Convert the slot-local DC and CAN-FD interface into the module-local wiring.
- Keep carrier-side interfaces reusable across all backplane slots.

Assembly notes:

- `J1` and `MOD1` are intentionally DNP/excluded from the PCB assembly BOM. The
  user installs and solders the right-angle XT30PW(2+2) connector and SW3538
  module after assembly.
- The housekeeping converter shares the backplane's LM5164DDAR regulator and
  PSPMAA0805-101M-ANP 100 µH inductor.
- The checked-in PCB is temporarily missing L1 so its new footprint can be
  placed and routed manually. The existing production outputs describe the
  previous completed layout and must be regenerated after that work.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
- `hardware-handoff.md`
- `datasheet-audit.md`
