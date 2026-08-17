# ADR 0004: Rev B Slot Input-Power Gates

- Status: Accepted
- Date: 2026-08-16

## Context

Backplane Backpack Rev A cannot remove input power from an individual SW3538
module. Emergency shutdown and ordinary disable therefore depend on a responsive
SW3538 and shared I2C path. The next hardware prototype adds six high-side FETs on
the backplane. Layout work moved the TCA9548A mux onto the main backplane and
replaced the serial shift register with a PCA9554 I2C GPIO expander. Each FET
disconnects the entire module input, including the SW3538.

## Decision

Rev B is a separate, mutually exclusive firmware board feature. Its logical port
capacity remains eight and its supported bitmap remains `0x3F`. Logical ports
`0..5` map directly to PCA9554 outputs `P0..P5`; unused `P6/P7` remain inputs. The
expander address pins are all low, giving 7-bit address `0x20`. It is upstream of
the TCA9548A at `0x70`, so power control never depends on selecting a mux channel.

The PCA9554 powers up with all pins as inputs and its output latch high. External
gate pull-downs hold all slots off in that state. Firmware first writes the output
latch to `0x00`, then writes configuration `0xC0`, making only `P0..P5` outputs.
Every later change writes the complete shadow byte and keeps bits `6..7` clear.
The task owning I2C2 serializes PCA9554 power operations, TCA9548A selection, and
SW3538 access. It polls emergency power-off first, then per-port power-off, before
other bus work. The physical backpack boundary carries only upstream SDA/SCL,
duplicated 3.3 V, and duplicated ground. Mux channels, power outputs, FET gates,
and carrier connections are entirely backplane-local.

An enable policy is durably persisted before its FET may turn on. Restored ports
are powered sequentially. After a provisional settling delay, firmware probes and
configures the newly powered SW3538. An initial probe failure, removal, or policy-
application failure schedules physical power-off. Clearing the emergency latch
does not itself restore any port.

## Consequences

Rev B provides a shutdown path independent of SW3538 responsiveness, but no longer
independent of upstream I2C health. A stuck upstream bus can delay or prevent a
PCA9554 cutoff write. The expander has no asynchronous output-enable or reset pin;
if only the MCU resets while the PCA9554 remains powered, previously enabled
outputs may remain enabled until startup firmware successfully writes all-off.
Complete power loss returns its pins to inputs, where the external pull-downs hold
the gates off. A future revision needs a hardware disable/reset path if reset-time
and bus-fault shutdown must be guaranteed.

Gated-off ports cannot be inventoried over I2C, so module presence is unknown
until a port is intentionally powered. Input switching also erases SW3538 state,
requiring policy reapplication after every power cycle.

Because the FET removes power from the SW3538 itself, firmware cannot configure
the controller before energizing it. The SW3538 hardware/default behavior still
exists between FET-on and completed policy application. Hardware bring-up must
measure this interval, rail settling, emergency latency, and startup inrush. A
separate always-powered controller supply or source inhibit would be required to
eliminate the interval.

The PCA9554 provides commanded state but no physical FET or rail readback. A
future revision may add power-good/current feedback if verified physical state is
required. The backplane-local TCA9548A reset is pulled inactive and is not routed
to the backpack, so Rev B firmware has no hardware mux-reset recovery action.
