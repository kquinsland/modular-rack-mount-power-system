# Controller PCB

KiCad project for the single central controller board.

Responsibilities:

- Host the WT32-ETH01 ESP32/Ethernet module.
- Drive and monitor one 3-wire 12 V fan.
- Accept regulated 5 V and 12 V controller rails.
- Provide upstream I2C/3.3 V and addressable-LED/5 V links to the first backplane.

The current PCB is at the same placement-only stage as the backplane: footprints
are present, but the board outline and routing are still open mechanical work.

Relevant docs:

- `../../../docs/system-overview.md`
- `../../../docs/interfaces.md`
- `../../../docs/power-budget.md`
