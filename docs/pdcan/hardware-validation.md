# Backplane Backpack Hardware Validation

- Status: waiting for Rev A and Rev B prototype hardware
- Scope: items that cross-compilation, pure tests, fake buses, and simulated
  flash cannot prove

This is the bring-up boundary for the current firmware. A checked box requires a
recorded observation from the target board; successful compilation is not enough.

## Boot and Safety States

- [ ] Measure power-on-to-fan-full-speed behavior for the 3-wire assembly.
- [ ] Confirm CAN transceiver standby remains asserted until FDCAN configuration
  is complete.
- [ ] Measure the SW3538 default advertisement and the interval before stored
  policy is applied.
- [ ] Confirm and document that Rev A cannot hard-limit a port during that
  initialization interval.
- [ ] On Rev B cold power-up, verify PCA9554 `P0..P5` remain effectively low
  through the input-to-output initialization sequence and every FET remains off
  through flash loading and a persisted emergency boot.
- [ ] On Rev B MCU-only reset, measure how long previously enabled PCA9554 outputs
  remain active before startup all-off; verify and document that the expander has
  no reset-time hardware cutoff.
- [ ] Measure the Rev B input rail rise and replace the provisional 10 ms
  settling delay with a reviewed bound.
- [ ] Measure the SW3538 default behavior between Rev B FET-on and successful
  policy application; confirm that whole-module switching does not eliminate
  this interval.
- [ ] Measure emergency-command-to-PCA9554-write, expander-output-to-gate, and
  gate-to-module-rail shutdown under idle, I2C-busy, rail-settling, and watchdog-
  reset conditions.
- [ ] Verify PCA9554 `P0..P5` map to logical ports `0..5`, `P6/P7` remain inputs,
  and whole-register updates do not glitch another port.
- [ ] Verify sequential restored-policy startup keeps aggregate inrush within
  the backplane and regulator limits.
- [ ] Verify a persisted emergency latch survives full power loss and watchdog reset.
- [ ] Verify explicit CLI acknowledgement clears the latch durably without
  enabling any port.
- [ ] Reassert emergency during clear erase, payload, and commit phases; verify
  the clear is rejected, runtime stays latched, and no emergency success is
  reported before the re-latch record is durable. Cut power at each boundary and
  record which last-committed state boots.

## Clock, CAN, and Watchdog

- [ ] Measure HSI accuracy over expected voltage and temperature against
  1/2 Mbit CAN-FD timing margins.
- [ ] Verify 29-bit extended CAN-FD traffic with BRS on a physical bus and a
  second node.
- [ ] Validate the provisional 500 ms `NODE_CLAIM` window with startup skew,
  duplicate Node IDs, collision-token retries, and representative bus load.
- [ ] Confirm firmware rejects classic/non-BRS traffic and transmits every draft
  payload length (`8`, `12`, `16`, `20`, `24`, and `32` bytes) correctly.
- [ ] Exercise error-passive, bus-off, recovery, congestion, and transceiver
  standby behavior.
- [ ] Trigger and verify each reachable RCC reset-cause flag, then confirm every
  mandatory stalled task causes an independent-watchdog reset reported in the
  next heartbeat.
- [ ] Replace provisional watchdog/progress windows with measured, reviewed limits.
- [ ] Measure worst-case interrupt latency and task stack high-water marks.

## I2C, Mux, and Hot-Swap

- [ ] Confirm the Rev B `i2c_backpack` boundary carries only duplicated 3.3 V,
  duplicated ground, upstream SDA, and upstream SCL with the documented pinout.
- [ ] Confirm I2C2 pin mapping, 100/400 kHz operation, upstream/downstream
  pull-ups, and TCA9548A address/power-on timing.
- [ ] Confirm the upstream PCA9554 responds at `0x20`, the TCA9548A responds at
  `0x70`, and the assembled topology contains one of each on the backplane.
- [ ] Confirm TCA9548A reset is locally pulled high and that no reset, interrupt,
  channel, or FET-control signal crosses the backpack boundary.
- [ ] Inject stuck upstream SDA/SCL while one or more FETs are on; record that
  firmware cannot guarantee cutoff and verify fault/status reporting.
- [ ] Demonstrate that ports 6 and 7 are never selected on Rev A.
- [ ] Demonstrate that Rev B never probes a gated-off port and reports physical
  presence as unknown until an explicit enable powers the module.
- [ ] Verify an initial Rev B NACK, policy failure, or later module-removal NACK
  turns the affected FET off without starving other ports.
- [ ] Characterize an empty-slot NACK and insertion/removal during every
  transaction phase.
- [ ] Verify Embassy I2C cancellation/timeout behavior leaves the peripheral reusable.
- [ ] Inject stuck SDA/SCL upstream and downstream of the mux and record bounded
  recovery, including Rev B's lack of a firmware-controlled mux reset.
- [ ] Define and validate safe SW3538 bank recovery after MCU reset or an
  interrupted banked write.
- [ ] Confirm one failed/stuck port cannot starve probes or operations on
  another port.

## SW3538 One-Port Capability Spike

- [ ] Read identity and known-value registers using registry revision `RG108_3_v1.2`.
- [ ] Validate write-unlock, extended-bank entry, and return-to-base sequences.
- [ ] Validate connection, protocol, contract, fault, and ADC interpretations.
- [ ] Validate enable, disable, CC-undriven, source-capability resend, and
  renegotiation semantics.
- [ ] Prove every advertised fixed PDO stays at or below 20 V, 5 A, and 100 W.
- [ ] Confirm PPS, EPR, and proprietary/nonstandard 7 A behavior remain
  unavailable in v1.
- [ ] Determine whether current-limit programming protects existing contracts
  or only new negotiation.
- [ ] Record behavior after removal or reset during each multi-register policy update.

## Flash, Fan, and Status LED

- [ ] Confirm the 2 KiB erase and 8-byte write geometry against STM32 reference
  material and silicon.
- [ ] Measure erase/program duration and interrupt/watchdog interaction.
- [ ] Interrupt power during erase, payload programming, and commit-marker programming.
- [ ] Validate 3-wire and 4-wire fan polarity/frequency, boot state, tach
  scaling, and stall detection.
- [ ] Measure the TIM15/DMA WS2812 waveform and reset interval at PA2/LED input.

## Evidence to Record

For each result, record board revision/serial, firmware commit, module and fan part
numbers, instrumentation, ambient/input conditions, observed min/max/typical
values, and pass/fail reasoning. Promote a provisional constant only when the
measurement and its safety margin are reviewed.
