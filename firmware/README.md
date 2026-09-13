# Firmware

This Cargo workspace targets the finalized Generation-2 hardware. Every backplane
and carrier is an independent STM32C092GCU6 CAN-FD node. I2C is board-local; the
host addresses nodes directly for control, telemetry, commissioning, and
application firmware updates.

## Workspace

- `backplane/`: backplane hardware contract and Embassy application target.
- `carrier/`: current SW3538 carrier contract and Embassy application target.
- `bootloader/`: common Embassy Boot swap/rollback bootloader.
- `crates/pdcan-types/`: shared role, capability, policy, telemetry, and update types.
- `crates/pdcan-protocol/`: strict CAN-FD codecs and message catalog.
- `crates/pdcan-core/`: transport-independent safety, commissioning, and update state machines.
- `crates/pdcan-drivers/`: INA237, DS18B20, and SW3538 drivers plus flash records.
- `crates/pdcan-artifact/`: versioned, signed-ready application update bundles.
- `tools/pdcan/`: Clap-based SocketCAN host utility with `clap_schema` discovery.
- `tools/pdcan-sim/`: deterministic Generation-2 backplane/carrier simulator.
- `xtask/`: CI, embedded builds, size gates, bundle construction, and DBC generation.

Firmware retains manual binary codecs; **Deku is not adopted**. See
[decision 0002](../docs/decisions/0002-do-not-use-deku-in-firmware.md) for the
rationale and supporting tests. The [benchmark report](docs/pdcan/deku-benchmark.md)
preserves the completed POWER and carrier-policy trial and reproduction commands.

## Commands

Run Cargo commands from this directory:

```sh
cargo xtask ci
cargo xtask firmware build backplane --release
cargo xtask firmware build carrier --release
cargo xtask firmware build bootloader --release
cargo xtask dbc --check
cargo run --package pdcan -- --help
cargo run --package pdcan -- schema --full
cargo run --package pdcan-sim -- --help
```

Build an unsigned application bundle after converting the linked application to
the raw ACTIVE-partition image expected by Embassy Boot:

```sh
cargo xtask firmware bundle carrier 1.0.0 carrier.bin carrier-1.0.0.pdcan
cargo run --package pdcan -- firmware inspect carrier-1.0.0.pdcan
```

Only Linux builds open SocketCAN interfaces. Host tests, artifact inspection,
Clap help, and schema discovery work on macOS. Embedded binaries use
`thumbv6m-none-eabi` and the `stm32c092gc` Embassy feature.

## Safety and update contract

Carrier outputs start disabled. A persisted enable is restored only after local
hardware checks and a deterministic UID-derived startup delay. Emergency and
fault handling always override policy. Backplane fans start at the safe 100%
duty state.

The host stages an image directly to the target node over CAN, verifies its
SHA-256 digest, and activates it separately. The current SW3538 profile advertises
`interrupt`, so `pdcan firmware activate` and the convenience `update` command
require `--allow-interruption`. A future profile may advertise `live` only after
its complete activation, trial-boot, failure, and rollback sequence has passed
profile-specific hardware-in-the-loop continuity testing.

The bootloader itself is SWD-only. Initial unsigned bundles provide transfer
integrity, not sender authentication; the artifact reserves Ed25519 algorithm,
key-ID, and signature fields for later enforcement.

See [plan.md](plan.md), [the firmware architecture](docs/pdcan/firmware-architecture.md),
and [the protocol specification](docs/pdcan/protocol.md).
