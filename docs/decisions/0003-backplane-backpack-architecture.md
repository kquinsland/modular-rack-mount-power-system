# 0003: STM32 CAN-FD Backplane Backpack

## Status

Accepted. Supersedes decision 0002.

## Decision

The active controller is one STM32C092-based Backplane Backpack per managed
backplane. Backpack nodes communicate with hosts over a trusted CAN-FD bus and own
their local upstream I2C path, main-backplane TCA9548A/PCA9554 devices, downstream
SW3538 transactions, fan, and status LED.

For Rev B, the backpack/backplane control boundary is intentionally only upstream
SDA/SCL plus duplicated 3.3 V and ground. The backplane owns mux channels, power
outputs/FETs, slot-local protection and pull-ups, and carrier connectors.

The firmware and PDCAN data model support eight zero-based logical ports. Rev A
physically exposes ports 0 through 5 and advertises a supported-port bitmap of
`0x3F`; its mux channels 6 and 7 are test pads and are never probed as ports.

The repository root is the Cargo workspace. Reusable `no_std` types, pure state
management, protocol encoding, and generic drivers live under `crates/`;
board-specific firmware remains under `firmware/backplane-backpack/`; Linux host
tools live under `tools/`.

The WT32 controller architecture from decision 0002 is no longer active. Existing
hardware sources remain as history, but firmware and current system/interface
documentation do not target them. Firmware for other CAN hosts is outside scope;
the Linux `pdcan` tool is in scope.

## Rationale

Giving each backplane a local deterministic controller keeps the hot-swappable,
fixed-address PD buses short and gives one task exclusive ownership of mux and I2C
recovery. CAN-FD supports several backpacks and independent development/operations
hosts without extending I2C or addressable-LED wiring between boards.

Separating the eight-port logical capacity from Rev A's six populated connectors
avoids embedding the first board's population limit into the protocol, persistent
format, core scheduler, and host tooling.

## Consequences

- Decision 0002 remains in the history but is superseded.
- Board revision is a compile-time firmware selection and is reported at runtime.
- PDCAN must distinguish unsupported ports from absent modules.
- More than one command host is supported through requester and request identities.
- Rev A emergency shutdown is best effort through I2C; its persistent latch is not
  a safety-rated independent power cutoff.
- Rev A cannot guarantee a stored firmware power ceiling before a module has been
  initialized. Per-port high-side isolation for the Rev B prototype is specified
  by [ADR 0004](0004-revb-slot-power-gates.md).
