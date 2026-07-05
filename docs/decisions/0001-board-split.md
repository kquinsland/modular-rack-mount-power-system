# 0001: Backplane and Carrier Board Split

## Decision

The project starts with two KiCad PCB projects:

- `backplane`: shared DC distribution, carrier connectors, and I2C mux.
- `carrier`: one slot module carrying a fixed-address DC/USB-C PD module.

## Rationale

The carrier module can be duplicated across slots, while the backplane owns slot count, mechanical spacing, power distribution, and I2C address isolation.

The I2C mux belongs on the backplane because the address conflict exists only when multiple carriers are installed together.

## Consequences

- Each physical PCB has its own KiCad project under `hardware/boards/`.
- Shared symbols, footprints, and 3D models live under `hardware/libraries/`.
- Backplane-to-carrier pinout decisions are documented in `docs/interfaces.md`.
