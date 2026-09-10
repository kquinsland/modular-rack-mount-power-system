# `pdcan` CLI

- Status: Linux/SocketCAN pre-v1 implementation
- Default interface: `can0`
- Default requester: `15`
- Success exit status: `0`
- Usage, validation, timeout, or node rejection status: `2`

Sources live in `firmware/tools/pdcan/` and `firmware/tools/pdcan-sim/`.
From the repository's `firmware/` directory, build with
`cargo build --package pdcan --package pdcan-sim --release`, or use
`cargo run --package pdcan -- --help`. The commands below assume the binaries
from `firmware/target/release/` are on your `PATH`.

Run `pdcan --help` for the complete option list. Common request options include
`--interface`, `--requester`, `--request-id`, `--timeout-ms`, `--retries`, and
`--json`. Retries preserve the exact request identity and payload so firmware can
deduplicate them safely.

JSON mode emits one JSON object per line. Every object has a stable `kind`
discriminator; multi-part commands such as `status` emit a correlated
`command_response` followed by `port_state`. The pre-v1 machine-output contract
is [`json-schema.json`](json-schema.json).
The 64-bit `request_id` is a decimal string in JSON so JavaScript consumers do not
silently lose integer precision.

## Commissioning

```text
pdcan scan --json
pdcan info 3 --json
pdcan identify 00112233445566778899aabb --seconds 30
pdcan assign 00112233445566778899aabb 3
pdcan clear-node 00112233445566778899aabb
```

Scan uses three nonce-rotated rounds by default and merges responses by UID.
`info` performs the same UID-safe discovery and filters the result by Node ID;
it therefore reports every conflicting UID rather than trusting an ambiguous
operational address.
Assignment refuses a Node ID reported by another UID unless the operator passes
`--force`. See [commissioning.md](commissioning.md) for the state/conflict model.

## Port and Fan Policy

Ports use explicit zero-based `NODE.PORT` syntax:

```text
pdcan status 3.0 --json
pdcan set-policy 3.0 --enabled \
  --max-voltage-mv 20000 --max-current-ma 5000 --max-power-mw 100000
pdcan set-policy 3.0 --disabled
pdcan set-fan 3 --mode 4-wire --duty 65
```

The CLI validates the global 20 V, 5 A, and 100 W caps before transmitting.
Firmware validates again and rejects Rev A ports 6 and 7 as unsupported.

`status` waits for both its correlated command result and the matching typed
`PORT_STATE` snapshot. Desired/module/emergency fields are available now; contract,
port-power telemetry and detailed SW3538 fault fields remain hardware-spike
work. `monitor` decodes `BOARD_TEMPERATURE` reports as PCB temperature in
degrees Celsius (and as signed centi-degrees Celsius in JSON mode).

## Emergency Operations

```text
pdcan emergency-disable all
pdcan emergency-disable 3
pdcan acknowledge-resolved 3
```

`acknowledge-resolved` is intentionally not named “enable” or “clear fault.” It
is the operator's explicit declaration that the dangerous condition has been
resolved and normal operation may be allowed again. Neither command enables a
port. Success means the corresponding latch state is durable.

## Monitoring

```text
pdcan monitor
pdcan monitor --requests --json
pdcan monitor --requester 4
```

Monitoring shows traffic from every requester by default, including requests not
originated by this `pdcan` process. The filter changes display only. Commissioning
IDs are shown with their 12-bit token rather than mislabeling it as Node/target.

## Simulator and `vcan`

```text
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0
pdcan-sim --interface vcan0 --ports 6 --node none
pdcan scan --interface vcan0
```

`pdcan-sim` can model six or eight supported ports and may start commissioned or
uncommissioned. Pure tests also cover persistence retries/power cycles, request-ID
reuse, unsupported ports, and duplicate claims. CI creates an isolated `vcan0` and
runs the full scan → identify → assign → policy/fan/status → emergency/acknowledge
→ clear flow.

`vcan` proves host framing, correlation, parsing, and workflow behavior. It does
not prove physical CAN arbitration, timing, error frames, transceiver behavior,
or the target FDCAN peripheral.

The CLI and simulator transmit and accept only extended CAN-FD frames with BRS,
matching the firmware's strict transport gate. Classic CAN, FD without BRS, and
standard-ID frames are ignored.
