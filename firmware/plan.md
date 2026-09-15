# Generation-2 Firmware, CAN Update, and Documentation Overhaul

## Implementation status — 2026-09-10

The architecture break and reusable foundations are implemented:

- The muxed backpack target and obsolete TCA9548A, PCA9554, and TMP102 drivers
  are removed. Separate backplane, carrier, and common Embassy Boot images now
  target STM32C092GCU6 and the finalized flash map.
- Generation-2 roles, all four carrier profiles, capabilities, direct-node
  control, telemetry, commissioning, optional binding, and live/interrupt update
  impact are represented in shared types, codecs, generated DBC, simulator, and
  host tooling.
- INA237, DS18B20, and revised SW3538 drivers have host-side tests. Board crates
  contain production-netlist pin contracts and fail-safe initial GPIO states.
- Firmware artifacts, SHA-256 verification, update sessions, 48-byte chunks,
  cumulative ACKs, retry/idempotency behavior, staging/activation separation,
  and interruption acknowledgement are implemented through the host, protocol,
  core state machine, and simulator.
- The common bootloader builds in 16 KiB. Both applications open their real
  Embassy Boot DFU/state partitions and read boot state. They deliberately do
  not call `mark_booted` until the required health gates exist.
- `pdcan` uses Clap and `clap_schema`; `pdcan schema` exposes the same command
  contract used for parsing. Typed `thiserror` operational failures have stable
  human/JSON categories and exit statuses.
- Active engineering docs, the public site, ADRs, commands, connector references,
  and validation checklist now describe Generation 2 only.

The embedded backplane/carrier binaries are still safe scaffolds, not complete
runtime firmware. Remaining implementation work is the Embassy task graph and
FDCAN transport, durable configuration/staged-manifest storage, real
`FirmwareUpdater` chunk writes/hash/`mark_updated`, bounded trial health and
`mark_booted`, board peripheral integration, signature metadata on the CAN
transport, Linux vcan scenarios, and first-article/HIL validation.

One non-electrical KiCad annotation also remains: the backplane control-sheet
note pairs PA8/PB8 with the wrong fan labels even though the actual nets and
exported IPC netlist are correct. Correct that note through Konnect when its
schematic write tools are available; do not text-edit the `.kicad_sch` file.

## Summary

The replacement architecture will use:

- One STM32C092GCU6/CAN-FD node per backplane.
- One STM32C092GCU6/CAN-FD node per carrier.
- Direct host-to-node control and firmware updates; backplanes never proxy carrier traffic or updates.
- Local I2C only—no inter-board I2C or mux.
- Optional manual carrier binding by backplane UID and zero-based slot.
- Autonomous local safety and persisted configuration when no host is present.
- A carrier-profile update-impact declaration of `live` or `interrupt`; the current SW3538 carrier is `interrupt`.
- Firmware updates staged by the running application and installed with Embassy Boot's power-fail-safe swap, trial boot, and rollback behavior. Embassy Boot itself provides no networking, so PDCAN supplies the transport. See the [Embassy Boot 0.7.0 documentation](https://docs.embassy.dev/embassy-boot/0.7.0/default/index.html).

## Current Drift

### Firmware

- It targets the retired STM32C092FCP6 backpack rather than the active STM32C092GCU6.
- It models one board owning six/eight muxed PD ports instead of independent backplane and carrier nodes.
- TCA9548A, PCA9554, mux scheduling, slot epochs, and eight-port configuration are obsolete.
- Current pin allocation is incorrect:
  - Firmware CAN: `PA11/PA12`; active boards: `PB0/PB1`.
  - Firmware expects CAN standby on `PA4`; active transceivers have standby tied low.
  - Firmware has one fan on `PA0/PA1`; the backplane has two channels on `PA2/PB8` and `PA0/PA8`.
  - Firmware LED: `PA2`; backplane LED: `PA1`; carrier LED: `PB8`.
- The firmware expects TMP102 rather than the backplane's DS18B20 and INA237. It also does not report the STM32 or INA237 internal die-temperature measurements available on both board types. On the backplane, the STM32 and INA237 are intentionally placed on opposite sides of the PCB so their readings can help characterize thermal spread.
- No carrier target implements `PD_ENABLE`, `PD_PGOOD`, `PD_IRQ`, `BUCK_PGOOD`, INA237 telemetry, the button, or local SW3538 control.
- Protocol and CLI assume `NODE.PORT`, one fan, and a supported-port bitmap. They cannot express distributed carrier nodes, two fans, capability-dependent carriers, or distinct aggregate/input/PD telemetry.
- Shared types hard-code the current SW3538's 20 V/5 A/100 W limits instead of obtaining limits from the carrier profile.
- UID commissioning, request correlation, flash journaling, CAN encoding, semantic LED rendering, host tooling, and simulator structure remain reusable.
- Ninety-three targeted tests pass. Full local CI is not macOS-compatible because it unconditionally builds SocketCAN, and the Rust/toolchain pins are inconsistent.

### Documentation

- The broad Gen-2 topology is correct, but detailed connector and pin references have drifted from the live netlists.
- DC input is `J8`, external CAN is `J7`, and slots are `J1`–`J6`; older references remain elsewhere.
- Backplane LED and fan pin documentation is stale.
- CAN routing constraints disagree between interface documents and current board guidance.
- Some pages describe the PCB as unfinished although its board README says it is routed.
- ADRs 0001–0004 still describe the retired split backpack architecture. Their useful history remains in Git, but the checked-in documents no longer need to be retained.
- Firmware/PDCAN documentation remains almost entirely backpack-specific even though that functionality has been merged into and refined on the consolidated backplane.

## Implementation Changes

### 1. Record the Gen-2 hardware contract

- Delete ADRs 0001–0004 and restart the decision log with a new `0001` describing the active Gen-2 architecture. Git history remains the record of the retired designs.
- Define independent backplane/carrier nodes, local-only I2C, direct carrier addressing, optional manual slot binding, autonomous safety, and external coordination.
- Define four carrier profiles:
  - Current SW3538 carrier: switch, input monitoring, PD control/status, 100 W policy ceiling.
  - Future basic carrier: switch and load monitoring, without negotiated USB-C visibility.
  - Future accessory carrier: capability-described fans, sensors, or other light loads, without assuming that it is a power supply.
  - Future 240 W carrier: reserved capability profile pending actual hardware.
- Make profiles capability-based rather than branching protocol behavior solely on profile names. Capabilities describe switchable outputs, load monitoring, PD negotiation, fans, temperature sensors, auxiliary sensors, and firmware-update impact.
- Correct active hardware, interface, overview, and site documentation from exported KiCad netlists.

### 2. Replace the backpack protocol model

- Make a clean protocol-major break; compatibility with the existing draft is not required.
- Advertise node role, hardware profile, firmware version, bootloader version, functional capabilities, and firmware-update impact (`live` or `interrupt`).
- Address carriers directly by commissioned Node ID.
- Persist optional `{backplane_uid, slot_index}` metadata on carriers. Clients resolve `backplane.slot` aliases to carrier Node IDs; bindings remain descriptive and electrically unverifiable.
- Add capability-specific operations:
  - Carrier enable/disable and local policy.
  - Optional PD limits and negotiated-contract status.
  - Backplane fan 0/1 control.
  - Aggregate backplane power and carrier-local input power telemetry.
  - Separate STM32 die, INA237 die, and external 1-Wire temperature telemetry, including sensor identity and source.
  - Power-path, accessory, fan, fault, update, and boot-state reporting.
- Keep UID discovery, Node-ID commissioning, request IDs, duplicate-address handling, version reporting, and broadcast emergency disable.
- Regenerate the DBC, JSON schema, simulator definitions, CLI help, and protocol documentation.

### 3. Split board firmware

- Replace `backplane-backpack-firmware` with:
  - `backplane-firmware`
  - `carrier-firmware`
  - A small common Embassy Boot bootloader binary.
- Target STM32C092GCU6 throughout.
- Share CAN transport, commissioning, persistence, update handling, watchdog supervision, and LED rendering.
- Implement separate state machines:
  - Backplane: aggregate INA237, two safe-default fans, tach inputs, multi-drop DS18B20 discovery, STM32 and INA237 die temperatures, LED, and health.
  - Carrier: one protected output, local INA237, STM32 and INA237 die temperatures, hot-swap signals, persisted policy, emergency latch, LED, and optional PD backend.
- Add INA237 and DS18B20 drivers. Remove active use of TCA9548A, PCA9554, and TMP102.
- Implement the current SW3538 carrier only; expose capability-driven abstractions for basic, accessory, and 240 W profiles without inventing unsupported hardware behavior.
- Restore a persisted carrier enable only after local checks and a deterministic UID-derived startup delay. Emergency and fault states always dominate.
- On broadcast emergency, carriers disable their local power path; backplanes force cooling to its safe state and report the event.

### 4. Add host-to-node CAN firmware updates

- Use `embassy-boot-stm32` and its `FirmwareUpdater` API in both applications. The application stages and verifies DFU storage; a separate activation command calls `mark_updated` and resets, after which the bootloader swaps images. See the [FirmwareUpdater API](https://docs.embassy.dev/embassy-boot/0.7.0/default/struct.FirmwareUpdater.html).
- Never route updates through a backplane. The host addresses and updates each backplane or carrier node directly.
- Reject broadcast updates and images whose role, hardware profile, MCU family, or partition-layout version does not match the target.
- Define update impact precisely:
  - `live`: the profile's hardware and firmware guarantee that its declared service remains available across staging, bootloader swap, reset, trial boot, and rollback.
  - `interrupt`: staging may occur while the service remains active, but activation resets the node and may interrupt its output or accessory function.
- Advertise `live` only after profile-specific HIL proves continuity across the complete activation and rollback sequence. A normal in-application download alone does not qualify a profile as `live`.
- Mark the current SW3538 carrier as `interrupt` because its power path is intentionally default-off while the MCU resets. Future hardware may advertise `live` if it can safely hold or independently control its load across reset.
- Add PDCAN update messages:
  - `FW_BEGIN`: manifest, image length, version, target identity, SHA-256 digest, and update session ID.
  - `FW_DATA`: session ID, byte offset, and 48-byte sequential data payload.
  - `FW_ACK`: cumulative next offset, state, and error information.
  - `FW_FINISH`: verify the completed image and persist its ready-to-activate manifest without rebooting.
  - `FW_ACTIVATE`: mark the verified image updated and perform the controlled reset.
  - `FW_ABORT` and `FW_STATUS`: cancellation and recovery diagnostics.
- Use an eight-frame transfer window with cumulative acknowledgements. Exact duplicate chunks are idempotent; gaps, conflicting retransmissions, oversize images, and writes outside DFU are rejected.
- On interrupted download, never mark the partial image updated. A surviving application session can resume from its cumulative offset; after reset, the host restarts staging from the beginning.
- During staging, keep an in-use carrier's output in its existing safe state. Pace internal-flash erase/program operations so CAN queues, fault inputs, and the watchdog remain serviced; any safety-critical fault still overrides the update and disables the output.
- `FW_FINISH` hashes the DFU image, verifies its manifest, records it as staged, and returns without calling `mark_updated` or resetting. The staged metadata must survive an ordinary reboot and be revalidated against the DFU contents before later activation.
- Activation is a separate operator action:
  - For `interrupt` profiles, the CLI requires an explicit `--allow-interruption` acknowledgement. Firmware gracefully disables the affected output/function, marks the staged image updated, acknowledges the command, and resets.
  - For `live` profiles, the node follows the profile's validated continuity mechanism before marking the image updated and resetting.
  - Backplanes never activate or schedule updates for carriers.
- If a safety fault develops during staging, abort or pause staging as appropriate, execute the normal safety response first, and report both conditions.
- Treat the first boot as a trial. Mark it booted only after flash/config validation, board-profile validation, CAN initialization, watchdog initialization, and critical local peripheral checks succeed. Host presence is not required. Failure or reset before confirmation permits rollback.
- Report running version, staged version, bootloader version, trial/confirmed/rollback state, last update result, and image digest through discovery/info.
- CAN updates cover application images only. Bootloader replacement and recovery from an invalid bootloader remain SWD operations.
- Extend `pdcan` with:
  - `firmware inspect IMAGE`
  - `firmware status NODE`
  - `firmware stage NODE IMAGE`
  - `firmware activate NODE [--allow-interruption]`
  - `firmware update NODE IMAGE`, as a stage-and-activate convenience only for `live` profiles or when `--allow-interruption` is supplied.
  - `firmware abort NODE`
- Update one node at a time by default. The CLI may expose explicit parallelism later, but backplanes do not coordinate updates.

### 5. Flash layout and artifact pipeline

- Replace the existing 248 KiB application layout with page-aligned BOOTLOADER, BOOTLOADER_STATE, ACTIVE, DFU, and persistent CONFIG partitions. Embassy requires DFU to be at least one erase page larger than ACTIVE.
- Begin with this 256 KiB/2 KiB-page layout:
  - Bootloader: 16 KiB
  - Bootloader state: 8 KiB
  - Active application: 110 KiB
  - DFU staging: 112 KiB
  - Reserved alignment page: 2 KiB
  - Persistent configuration: 8 KiB
- Verify erase geometry and actual bootloader size against the STM32C092GCU6 before freezing the linker scripts. If the bootloader exceeds 16 KiB, grow it by whole pages and reduce ACTIVE/DFU symmetrically while preserving `DFU >= ACTIVE + one page`.
- Set CI's application size budget below the 110 KiB ACTIVE limit with explicit headroom.
- Make CAN update support conditional on an early size-feasibility gate. Both release applications must fit a 110 KiB ACTIVE partition, with a target budget of at most 100 KiB to retain growth headroom. If either application cannot meet the 110 KiB hard limit after reasonable size optimization, remove Embassy Boot, DFU partitions, and CAN-update protocol support and restore a single-application/SWD layout.
- Generate role-specific signed-ready update bundles containing manifest, raw application image, SHA-256 digest, and an optional signature record.
- Initially accept unsigned bundles. Preserve signature algorithm/key-ID/signature fields in the artifact and protocol so Ed25519 enforcement can be enabled later without redesigning the update format.
- Document CAN update as a convenience feature for the closed, trusted bus. The initial SHA-256 check provides transfer integrity but not sender authenticity.

## Test and Acceptance Plan

- Protocol golden vectors for roles, capabilities, binding, update manifests, chunks, acknowledgements, retries, and rejection cases.
- State-machine tests for autonomous boot, staggered restoration, emergency handling, update-mode safe states, trial confirmation, failed confirmation, and rollback.
- Temperature tests must keep STM32 die, INA237 die, and each addressed DS18B20 sample distinct and must represent unavailable, stale, and invalid readings without substituting one source for another.
- Power-loss tests at every DFU erase/write boundary, before and after `mark_updated`, during swap, and before `mark_booted`.
- Verify wrong-role, wrong-profile, truncated, oversized, corrupted, stale-session, out-of-order, and conflicting-retry images are rejected.
- Confirm a partial transfer can never become bootable.
- Confirm the current `interrupt` carrier keeps a safe in-use output active during staging, requires explicit interruption acknowledgement, disables it for activation, and restores persisted policy only after successful trial confirmation and stagger.
- For any future `live` profile, validate uninterrupted declared service during staging, activation, bootloader swap, reset, successful trial boot, failed trial boot, and rollback before allowing that capability to be advertised.
- Driver tests for INA237, DS18B20, SW3538, and carrier power-path sequencing.
- Simulator scenarios with multiple backplanes and mixed carrier profiles, while updating one directly addressed node without disturbing peer traffic.
- Linux vcan tests for complete update, retry, abort, checksum failure, node disappearance, and post-reboot version discovery.
- Embedded builds and flash/RAM budgets for bootloader, backplane, and carrier images.
- HIL validation of CAN updates on both board types, including power interruption, watchdog reset, rollback, SWD recovery, and continued operation of unaffected CAN nodes.
- Documentation validation must ensure hardware mappings, protocol schemas, update artifacts, DBC, CLI help, and public-site guidance agree.

## Assumptions and Defaults

- STM32C092GCU6 is the common MCU.
- Updates are always host-to-node and never proxied.
- Application images are updateable over CAN; bootloaders are SWD-only.
- Existing protocol compatibility is discarded.
- Firmware signing is deferred, but the manifest remains signature-ready.
- SHA-256 integrity verification is mandatory even while unsigned images are accepted.
- The current SW3538 carrier advertises `interrupt`; no existing carrier advertises `live`.
- Staging and activation are separate persistent states and operator actions.
- Slot indexes remain zero-based.
- Binding uses immutable backplane UID rather than mutable Node ID.
- Nodes remain safe and functional without a continuously present host.
