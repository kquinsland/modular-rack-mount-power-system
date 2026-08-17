# Firmware

The active firmware target is the STM32C092FCP6 on the Backplane Backpack.
Board-specific embedded code lives under
[`backplane-backpack/`](backplane-backpack/), while reusable `no_std` types,
business logic, protocol code, and drivers live in the repository's root
`crates/` workspace.

The firmware and PDCAN protocol support eight logical ports. Backplane Backpack
Rev A exposes ports 0 through 5 and reports ports 6 and 7 as unsupported.
The Rev B prototype exposes the same six logical ports and adds per-slot input
power control through a PCA9554/high-side-FET stage on the shared upstream I2C
bus. The backpack connector carries only SDA/SCL plus duplicated 3.3 V and
ground; the backplane owns both I2C devices and all slot-local circuitry. Build
it explicitly with
`cargo xtask firmware build --board rev-b --release`.

The former WT32 controller firmware architecture is superseded and is not an
implementation target.

References:

- [`plan.md`](plan.md): reviewed implementation and repository integration plan.
- [`backplane-backpack/backplane-plan.md`](backplane-backpack/backplane-plan.md):
  original detailed design input.
- [`../docs/interfaces.md`](../docs/interfaces.md): active electrical and logical
  interface contract.
- [`../docs/pdcan/firmware-architecture.md`](../docs/pdcan/firmware-architecture.md):
  implemented task/peripheral boundaries.
- [`../docs/pdcan/hardware-validation.md`](../docs/pdcan/hardware-validation.md):
  explicit bring-up and HIL checklist.
