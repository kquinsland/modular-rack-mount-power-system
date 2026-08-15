# Firmware

The active firmware target is the STM32C092FCP6 on the Backplane Backpack.
Board-specific embedded code lives under
[`backplane-backpack/`](backplane-backpack/), while reusable `no_std` types,
business logic, protocol code, and drivers live in the repository's root
`crates/` workspace.

The firmware and PDCAN protocol support eight logical ports. Backplane Backpack
Rev A exposes ports 0 through 5 and reports ports 6 and 7 as unsupported.

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
