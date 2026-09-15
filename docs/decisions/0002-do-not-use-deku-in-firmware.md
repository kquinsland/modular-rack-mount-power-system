# 0002: Do Not Use Deku in Firmware

Status: Accepted 2026-09-13.

## Decision

Do not use Deku in the firmware. Retain the explicit, heap-free binary codecs in
`pdcan-protocol`, including their CAN identifier packing and semantic validation.
Do not enable the experimental `deku-codec` feature in production firmware builds.

The trial is complete and rejected for adoption. Its implementation and benchmark
artifacts on `codex/deku-codec-bench` (commit `3e4f021`) are evidence for this
decision, not a planned firmware dependency migration. The implementation is
archived as a patch alongside the report; it is not part of the firmware build. This decision concerns firmware; it does not select
a serialization library for unrelated host tooling.

## Trial and evidence

The trial started from `main` at
`9680cd3992865a311ab963fa6192fc00b31434e8`. It compared the manual codecs with
Deku 0.20.3 for `POWER` telemetry and the `SET_CARRIER_POLICY` body. Deku's default
features were disabled, excluding runtime `std`, `alloc`, and `bits`. Existing
header, length, boolean, reserved-field, and source validation remained explicit.

Both variants were built with Rust 1.98.1 / LLVM 22.1.8. Embedded builds targeted
`thumbv6m-none-eabi`; host timings ran on an AMD Ryzen 9 5950X under Linux.

| Evidence | Result |
| --- | --- |
| Workspace tests | All 69 tests passed in each configuration. |
| Independent byte vectors | Endianness, signed values, optional limits, invalid flags, reserved fields, and incorrect lengths passed. |
| Differential corpus | All 54,128 output records matched exactly, including encoded bytes, decoded values, and errors. |
| Repository checks | Standard firmware CI passed; Deku-enabled workspace Clippy and embedded probe Clippy also passed. |
| Linked codec probe, current release profile | Manual: 8,700 bytes flash; Deku: 21,136 bytes; increase: **12,436 bytes**. |
| Linked codec probe, `opt-level="z"`, LTO, one codegen unit | Manual: 5,380 bytes flash; Deku: 6,480 bytes; increase: **1,100 bytes**. |
| Static RAM in both probe profiles | 24 bytes for either implementation; no allocator required. |
| Host policy encoding | 22.26 ns manual versus 31.77 ns Deku: approximately **43% slower**. |
| Other host operations | Power encoding was similar, power decoding about 10% faster, policy decoding about 7% slower, and reserved-field rejection similar. |

The corpus included 4,096 valid examples per message type, exhaustive single-byte
mutations on selected examples, malformed lengths, CAN identifier bit flips, and
deterministic arbitrary inputs. Timings used 21 batches of 100,000 calls per
operation, warmup, and three process runs with alternating variant order.

The application entry points did not yet call the codecs, so ordinary application
sizes could not establish adoption cost. A separate linked probe retained the
encode/decode paths and existing control dispatcher. Static RAM excludes stack;
MCU execution time and stack high-water marks were not measured. Host timings
are not MCU latency measurements. The two probe profiles do not predict the cost
of every future message layout.

## Rationale

Deku met the tested correctness and heap-free requirements. The rejection follows
from the tradeoff for these small, fixed-layout messages: the reduction in manual
field packing is insufficient to justify the measured flash increase, additional
dependency and derive machinery, and lack of a consistent performance benefit.
Protocol validation still needs explicit code.

Size optimization substantially reduced the overhead, but still added 1,100 bytes
in the probe and required a different release profile. Changing the application's
release profile solely to accommodate these codecs is not justified. Retaining
the manual implementation preserves flash headroom and direct visibility into
the wire format.

## Consequences

- Continue implementing and validating firmware binary layouts explicitly.
- Preserve the trial, byte vectors, raw measurements, and reproduction procedure
  as evidence; the experimental feature is not a production configuration.
- Do not change release settings as part of this decision.
- Reconsider only through a new decision supported by materially different
  requirements or measurements, such as substantially more complex layouts and
  demonstrated gains under the intended firmware release profile.

## Evidence artifacts

- [Full benchmark report and reproduction commands](../../firmware/docs/pdcan/deku-benchmark.md)
- [Raw measurements, feature trees, and per-process timing ranges](../../firmware/docs/pdcan/deku-benchmark-results.json)
- [Archived trial patch](../../firmware/docs/pdcan/deku-trial.patch), containing the
  benchmark runner, host benchmark and corpus, independent byte-vector tests,
  embedded size probe, experimental layouts, and dependency changes needed to
  reproduce the trial from its base commit.
