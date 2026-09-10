# Firmware

The implemented firmware target is the STM32C092FCP6 on the legacy Backplane
Backpack. Its KiCad project is retired; the current hardware is the consolidated
[`backplane`](../hardware/boards/backplane/). This firmware has not yet been
ported to that board's pinout and direct-CAN slot architecture.
Board-specific embedded code lives under
[`backplane-backpack/`](backplane-backpack/), while reusable `no_std` types,
business logic, protocol code, and drivers live in [`crates/`](crates/).

## Workspace and commands

This directory is the Cargo workspace root. It contains the pinned toolchain,
Cargo configuration/lockfile, shared crates, [`tools/`](tools/) (`pdcan` and
`pdcan-sim`), [`xtask/`](xtask/), and [`docs/pdcan/`](docs/pdcan/) (including the
generated DBC). All direct Cargo commands below run from `firmware/`, not the
repository root:

```sh
cd firmware
mise install
cargo xtask ci
cargo run --package pdcan -- --help
cargo run --package pdcan-sim -- --help
cargo xtask dbc --check
```

The repository root retains thin `mise run firmware:check`,
`mise run firmware:build`, and `mise run firmware:dbc` shortcuts. Actual task
definitions and the Rust tool pin live in [`mise.toml`](mise.toml). The GitHub
Actions entry point must remain in `.github/workflows/firmware-rust.yml`; it runs
inside this workspace and caches `firmware/target/`. Cross-cutting hardware
contracts, datasheets, and architectural decisions remain in the repository's
top-level `docs/`.

`cargo xtask ci` runs host tests, both legacy board-feature test/Clippy builds,
DBC drift checks, and both embedded release builds with flash/RAM budgets. The
SocketCAN integration tests additionally require an existing virtual CAN
interface and `PDCAN_VCAN_INTERFACE=vcan0`; CI provisions one, while local runs
without that variable skip the interface-dependent tests. No physical CAN bus
or board is needed for the remaining checks.

## Legacy board selection

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
- [`docs/pdcan/firmware-architecture.md`](docs/pdcan/firmware-architecture.md):
  implemented task/peripheral boundaries.
- [`docs/pdcan/hardware-validation.md`](docs/pdcan/hardware-validation.md):
  explicit bring-up and HIL checklist.
