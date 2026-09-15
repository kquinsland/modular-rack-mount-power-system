# Generation-2 Firmware Hardware Validation

- Status: pending first-article firmware bring-up on the finalized boards
- Scope: observations that compilation, unit tests, simulation, and fake flash
  cannot establish

A checked item needs the board revision/serial, firmware commit, supply and
load conditions, instrument setup, measured result, and pass/fail rationale.

## Common MCU, boot, and safe state

- [ ] Confirm both boards boot the common STM32C092GCU6 bootloader and their
  role-specific application from the documented flash partitions.
- [ ] Confirm the bootloader never enables carrier power output or changes a
  backplane fan from its hardware-safe state.
- [ ] Measure reset-to-application and watchdog-reset behavior, including every
  RCC reset-cause flag that firmware reports.
- [ ] Cut power during configuration erase, payload programming, and commit;
  verify the last complete record is selected and outputs remain safe.
- [ ] Verify a persisted emergency latch survives full power loss and watchdog
  reset, and explicit acknowledgement clears it without enabling an output.
- [ ] Measure stack high-water marks, worst-case interrupt latency, and watchdog
  margins under maximum CAN, sensor, LED, and flash activity.

## CAN-FD network

- [ ] Verify STM32 PB0 RX/PB1 TX and the permanently enabled TCAN3413 on both
  boards at the selected nominal/data bit rates over the intended harness.
- [ ] Verify extended CAN-FD+BRS traffic among a host, at least one backplane,
  and multiple carriers; confirm classic, standard-ID, and non-BRS traffic is
  ignored.
- [ ] Validate the commissioning claim window with startup skew, duplicate Node
  IDs, token collisions/retries, multiple backplanes, and representative load.
- [ ] Exercise error-passive, bus-off, recovery, unplug/replug, missing
  termination, and excessive-stub cases without unsafe output transitions.
- [ ] Measure emergency-command latency to each carrier's `PD_ENABLE` low state
  under idle, saturated-bus, I2C-busy, flash-write, and watchdog conditions.
- [ ] Confirm loss of any backplane does not prevent the host from addressing a
  carrier which remains powered and attached to the CAN trunk.

## Backplane

- [ ] Confirm NeoPixel data is PA1 and validate waveform timing, reset interval,
  startup-off behavior, and electrical levels.
- [ ] Confirm fan 0 PWM/tach is PA2/PB8 and fan 1 PWM/tach is PA0/PA8.
- [ ] Validate 3-wire fan supply-PWM polarity/frequency, tach scaling, startup,
  stall detection, and the pair's combined load on the 1 A 12 V rail.
- [ ] Confirm I2C2 PA6/PA7 communicates only with INA237 address `0x40`, and INA
  alert on PA4 wakes/flags the expected path.
- [ ] Validate the 1 mOhm shunt in narrow-range mode across expected current,
  temperature, reverse-current, overrange, and alert conditions.
- [ ] Confirm DS18B20 1-Wire on PA15 with external power, including CRC failure,
  missing sensor, conversion timeout, unplug/replug, and long-cable behavior.
- [ ] Confirm PA3 12 V power-good gating and recovery behavior.
- [ ] Verify aggregate inrush and fan startup remain within supply/backplane
  limits when several independently booting carriers are inserted.

## Current SW3538 carrier

- [ ] Confirm PB3 `PD_ENABLE` is held low before peripheral initialization and
  remains low after reset, corrupt configuration, emergency, or unrecoverable
  fault.
- [ ] Confirm PA15 `PD_PGOOD`, PC6 `PD_IRQ`, PA8 `BUCK_PGOOD`, PB5 INA alert,
  PC15 user button, and PB8 NeoPixel against the production board.
- [ ] Confirm I2C1 PB6/PB7 reaches only the local INA237 and SW3538 circuitry;
  inject stuck SDA/SCL and verify bounded recovery or safe disable.
- [ ] Validate the 6 mOhm INA237 configuration across output operating range and
  compare measured current/power with calibrated instruments.
- [ ] Validate SW3538 identity, unlock/bank transitions, enable/disable,
  interrupt handling, contract/fault decoding, and return to the base bank after
  interrupted transactions.
- [ ] Prove advertised fixed PDOs remain at or below 20 V, 5 A, and 100 W and
  that unsupported PPS, EPR, and nonstandard 7 A behavior cannot be enabled.
- [ ] Measure the interval from enable through policy application and confirm
  failures or timeouts return `PD_ENABLE` low.
- [ ] Verify the SW3538 profile advertises `interrupt` update impact and that the
  host refuses activation without `--allow-interruption`.

## Firmware update and rollback

- [ ] Stage valid backplane and carrier bundles from the host over CAN and
  confirm target/profile/revision/layout mismatches are rejected before write.
- [ ] Drop, duplicate, reorder, corrupt, and delay data frames; verify cumulative
  ACK/retry behavior, byte-for-byte duplicate checks, gap rejection, and final
  SHA-256 verification.
- [ ] Reset or remove power during every erase/write/finish/activate boundary;
  verify the node boots either the last confirmed image or a fully verified
  staged image, never partial data.
- [ ] Confirm staging does not interrupt normal service for both update-impact
  profiles; activation follows the profile's declared live/interrupt contract.
- [ ] Boot an intentionally unhealthy candidate and verify Embassy Boot rollback
  after the bounded confirmation window.
- [ ] Boot a healthy candidate and verify `mark_booted`, durable running-version
  metadata, subsequent reset, and another update.
- [ ] Confirm a host can update a carrier while its backplane is absent; observe
  that no backplane-to-carrier updater traffic exists.

## Release evidence

Before calling firmware production-ready, archive CAN traces, firmware bundle
hashes, flash maps, power-interruption matrix, timing distributions, thermal and
load conditions, and all deviations. Promote provisional constants only after
the measured worst case and margin have been reviewed.
