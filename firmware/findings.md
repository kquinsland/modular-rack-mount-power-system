# Generation-2 firmware safety findings

Reviewed: 2026-09-15, against `main` at
`fa8474864fc9dd40337e75d8c992920626ada340`.

## Scope and implementation status

These findings cover the current independent backplane and carrier nodes.
The [carrier application](carrier/src/main.rs) and
[backplane application](backplane/src/main.rs) initialize safe GPIO states,
read Embassy Boot state, and then idle. The embedded runtime does not yet
execute the control core, service CAN, monitor peripherals, or persist
configuration. See the [implementation status](plan.md#implementation-status--2026-09-10).

The findings distinguish existing library shortcomings from requirements for
the unfinished runtime. Priorities describe their significance for operational
firmware; they do not imply that the current idle applications reproduce each
failure. Finding identifiers are retained for traceability to the earlier
review; only concerns applicable to Generation 2 are included.

## Existing library shortcomings

### F-04: shutdown overwrites observed output state before confirmation

**Priority: high. Status: present in the control core; runtime integration pending.**

In [the carrier controller](crates/pdcan-core/src/lib.rs),
`CarrierController::emergency_disable()` sets `output_power_good` to false and
the service state to `Disabled` before returning the hardware shutdown action.
This discards the last observed power-good state without new hardware feedback.
`observe_output_power_good(true)` while disabled also does not raise an
unexpected-on fault. The emergency fault flag itself is correctly asserted.

Keep the requested output state separate from observed feedback. Preserve the
last observation and its validity until a fresh sample arrives. Represent
shutdown pending, failed, or unknown explicitly; require the expected feedback
within a deadline and fault unexpected power-good while disabled. Establish
what the carrier's power-good signal can actually prove during hardware
validation rather than treating it as a measurement of every downstream rail.

Acceptance tests should cover delayed feedback, a failed shutdown, stale or
missing feedback, and power-good remaining asserted after emergency disable.

### F-07: configuration trust and durable emergency acknowledgement are incomplete

**Priority: high. Status: present in library interfaces; storage integration pending.**

[`config_record::select_newest()`](crates/pdcan-drivers/src/config_record.rs)
returns `None` whenever both records fail decoding. Its result does not
distinguish erased storage from corrupted previously initialized storage. There
is no application configuration loader yet, so handling that result as factory
defaults would introduce a loss-of-trust failure during integration.

Also, [`CarrierController::acknowledge_emergency()`](crates/pdcan-core/src/lib.rs)
clears the in-memory emergency latch and fault before returning
`PersistSettings`. The core has no durable-completion gate for this transition;
it can accept a subsequent enable policy before the acknowledgement is stored.

Define separate outcomes for virgin provisioning, valid configuration,
corruption, and read failure. Loss of trusted configuration must hold outputs
disabled and require explicit recovery. Keep emergency acknowledgement pending
until persistence has completed and readback has verified it. A failed write
must leave the effective latch set, and successful acknowledgement must not
enable the output by itself.

Acceptance tests should cover erased records, one valid record, two corrupt
records, read errors, failed acknowledgement writes, and reset at each commit
boundary. Verify persisted emergency behavior through real power interruption.

## Runtime implementation requirements

### F-01: pending enables must not survive emergency or safety invalidation

**Priority: high. Status: execution contract not yet implemented.**

[`CarrierAction::DriveOutput(bool)`](crates/pdcan-core/src/lib.rs) is a copyable
action without an operation generation or cancellation token. No embedded
dispatcher currently executes it. If the eventual runtime queues an enable,
then handles an emergency, that previously issued action must not later assert
`PD_ENABLE`.

Authorize each enable against current emergency, fault, and hardware-health
state immediately before actuation. Invalidate pending enables when safety
state changes, including actions delayed by configuration persistence. Define
the concurrency rules across every await between authorization and the GPIO
write. Test emergency and fault injection at those boundaries and prove that
obsolete enables cannot repower the output.

### F-02: local startup sequencing and aggregate inrush remain unimplemented

**Priority: high. Status: partial core support; no embedded startup sequence.**

The [core](crates/pdcan-core/src/lib.rs) provides
`carrier_startup_delay_ms()`, a deterministic UID-derived delay of 100–899 ms,
and `restore_after_startup_delay()`, which checks housekeeping health and the
emergency latch. The caller is responsible for actually waiting. Neither
function is integrated into the application. Different UIDs can produce the
same delay, so staggering does not guarantee nonoverlapping starts.

Implement per-carrier configuration/profile validation, startup delay, local
health checks, and a bounded enable/policy-application/verification sequence.
Any failure or timeout must return the output to a safe state. Emergency and
fault handling must remain available throughout startup.

Test each prerequisite and timeout in software. Measure the enable-to-policy
interval and aggregate inrush with multiple carriers, including delay
collisions and simultaneous insertion or power restoration.

### F-03: runtime measurement and fault response are absent

**Priority: high. Status: drivers exist; application monitoring is missing.**

The [SW3538 driver](crates/pdcan-drivers/src/sw3538.rs) exposes status and ADC
operations, and the [INA237 driver](crates/pdcan-drivers/src/ina237.rs) provides
local measurement support. The embedded applications do not invoke these
drivers, process their alerts, or implement continuous fault monitoring.
Validating a requested PD policy alone does not establish measured operating
conditions or enforce a response to a runtime fault.

Implement bounded sampling and interrupt handling, explicit sample validity
and age, fault classification, and prioritized safe-output responses. Telemetry
must distinguish unavailable, stale, and invalid measurements from healthy
observations. Verify fault response and latency under sensor errors and load.

### F-05: CAN receive responsiveness under transmit congestion is unproven

**Priority: high. Status: embedded FDCAN transport is missing.**

The applications have no FDCAN service yet. Host tooling, codecs, and simulation
do not establish whether MCU emergency reception remains responsive when
transmission cannot progress.

Keep receive independently serviceable, bound transmit waits, and prevent
normal traffic or full queues from blocking safety handling. Define handling
for replaceable telemetry, critical responses, error-passive, and bus-off.
Test no-ACK operation, saturation, full transmit queues, and bus recovery while
emergency traffic arrives. Measure command-to-output latency on hardware.

### F-06: watchdog supervision is missing

**Priority: high. Status: no application watchdog initialization or supervision.**

Neither embedded application initializes or feeds a watchdog. Their boot
confirmation comments explicitly defer watchdog and other health gates.

Supervise both task scheduling and service health. Track successful operations,
consecutive failures, and bounded recovery progress instead of accepting timer
ticks or repeated errors as proof of health. Legitimate host absence must not
itself trigger reset. Test stuck tasks and persistent peripheral failures,
then measure watchdog margins under concurrent CAN, sensor, and flash work.

### F-08: bounded local I2C and SW3538 recovery is missing

**Priority: high. Status: conservative driver failure handling; no recovery service.**

The [SW3538 driver](crates/pdcan-drivers/src/sw3538.rs) marks its bank state
unknown after a bus error and refuses subsequent operations requiring a known
bank. This avoids unsafe register assumptions, but the application has no
recovery procedure. `assume_base_bank()` is valid only after the caller has
independently established that the controller is in the base bank.

Implement bounded peripheral and bus recovery, retry/backoff, and an explicit
terminal safe-disable state. Establish a hardware-supported procedure for
restoring the controller's bank state; clearing the software state alone is
insufficient. Validate stuck SDA/SCL, interrupted bank transitions, persistent
failures, and recovery while an emergency is pending.

### F-10: durable configuration writes need verification and bounded retries

**Priority: medium. Status: record codec exists; configuration storage service is missing.**

[`config_record`](crates/pdcan-drivers/src/config_record.rs) encodes, decodes,
and selects records. It does not implement the application's flash write,
readback, or retry service. Reading Embassy Boot state in `main` does not
supply configuration persistence.

Preserve the previous valid record while writing a replacement, commit last,
then read back and decode the new record before reporting durable success.
Bound retries and backoff; expose a terminal storage fault. Couple effective
emergency acknowledgement to that verified completion as required by F-07.
Test write failures and interrupted commits with fake flash, followed by
power-cut tests on the MCU.

## Validation and limits

The relevance review ran the following command from `firmware/` at the reviewed
commit:

```sh
cargo test --locked -p pdcan-core -p pdcan-drivers -p backplane-firmware -p carrier-firmware
```

All 33 unit tests passed: 8 core, 19 driver, and 3 per board library. These
tests validate existing foundations; they do not establish the missing runtime
guarantees or the acceptance criteria above. No target rebuild or hardware
tests were performed for this review.

Hardware evidence is still required for power-good semantics, startup inrush,
emergency latency, I2C recovery, watchdog margins, and persistent state across
power interruption. Record it in the
[hardware validation checklist](docs/pdcan/hardware-validation.md).
