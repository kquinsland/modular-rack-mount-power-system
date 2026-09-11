# 0001: Generation-2 Direct CAN Node Architecture

## Decision

The product consists of backplanes and carriers connected to shared power and
CAN-FD trunks. Every board uses an STM32C092GCU6 and TCAN3413 and is an
independently commissioned PDCAN node.

A backplane distributes an unswitched input bus to six carrier connectors and
owns aggregate input monitoring, two fans, an external temperature sensor, and
one status NeoPixel. A carrier owns its power-output safety, local monitoring,
status NeoPixel, and profile-specific controller. Local I2C buses do not cross
board boundaries; there is no I2C mux.

The host addresses every node directly for discovery, configuration, telemetry,
and Embassy Boot application updates over CAN. A backplane does not proxy or
perform a carrier update. Each carrier profile declares whether firmware
activation is live or interrupts its service.

## Rationale

Direct CAN addressing matches the physical bus, removes fixed-address I2C
isolation and controller dependencies, allows multiple backplanes to share a
power supply, and permits carrier types to evolve without changing backplane
firmware. A common MCU and bootloader keep build, commissioning, recovery, and
update behavior consistent across roles.

## Consequences

- Operational identity is a persistent Node ID associated with the STM32 UID,
  not a backplane/port address.
- Optional backplane UID and slot binding is descriptive inventory metadata.
- Carrier behavior is capability/profile-driven. The initial SW3538 carrier is
  an interrupt-update profile; basic and future 240 W profiles may differ.
- Each node must enter its own safe state on boot, emergency, communication
  loss, invalid configuration, and failed update.
- Firmware bundles are target-specific and staged into a per-node DFU partition.
  Activation uses Embassy Boot trial/confirmation/rollback behavior.
- Previous research/prototype protocols, firmware targets, and hardware
  topologies are not compatibility requirements.
