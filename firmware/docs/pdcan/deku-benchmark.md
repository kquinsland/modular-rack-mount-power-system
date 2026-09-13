# Deku codec experiment

Measured on 2026-09-13 on branch `codex/deku-codec-bench`, based on
`main` at `9680cd3992865a311ab963fa6192fc00b31434e8`. The trial was preserved
as commit `3e4f021` and as [deku-trial.patch](deku-trial.patch). The implementation
and experimental Cargo feature are excluded from the firmware on `main`.

**Decision: do not use Deku in firmware; retain the manual codecs.**
[Decision 0002](../../../docs/decisions/0002-do-not-use-deku-in-firmware.md)
records the accepted outcome. Deku works without a heap and preserves the tested
wire behavior, but the measured costs do not justify adoption for these layouts.
The linked codec probe grows by 12,436 bytes with the current default release
profile, or 1,100 bytes with size optimization and LTO. This report preserves the
completed trial as evidence.

The experiment uses Deku 0.20.3 with default features disabled: no runtime `std`,
`alloc`, or `bits`. It changes `POWER` payload encoding/decoding and
`SET_CARRIER_POLICY` body encoding/limit decoding. Request IDs, CAN identifier
packing, and semantic validation remain in the existing codec. The default build
continues to use manual serialization.

The derived structs contain raw integers and fixed arrays. Boolean, reserved-byte,
source, header, and exact-length checks run in the original validation layer.
The fixed-layout helpers use `expect` only after the caller has established buffer
length; they introduce no parse assertions on unvalidated enum or boolean values.

## Correctness

- All 69 workspace tests pass with the default codecs and with `deku-codec`.
- New independent vectors cover little-endian fields, signed extrema, optional
  policy limits, invalid flags, reserved fields, and incorrect lengths.
- Both builds produce identical output for all 54,128 differential corpus records.
  The corpus includes 4,096 valid messages of each type, exhaustive single-byte
  replacements on four examples of each type, lengths 0–80, every CAN identifier
  bit flipped individually, and deterministic arbitrary inputs. Successful decoded
  values, encoded frames, and exact error variants are compared.
- Corpus SHA-256: `da462110e5835a25b8ddf316a192802f66a9c36521c3b15421953241b0cb6c32`.
- Standard `cargo xtask ci` passes on Rust 1.98.1, including formatting, default
  workspace tests, Clippy, DBC consistency, embedded checks, and firmware size gates.
- Workspace Clippy with Deku enabled and embedded Clippy for the size probe pass.

## Embedded size

Rust 1.98.1 / LLVM 22.1.8, `thumbv6m-none-eabi`. Values are bytes. Flash is
`text + data`; static RAM is `data + bss` from `llvm-size`.

| Image/profile | Manual flash | Deku flash | Change | Static RAM, both |
|---|---:|---:|---:|---:|
| Backplane, default release | 13,728 | 13,724 | -4 | 248 |
| Carrier, default release | 13,704 | 13,704 | +0 | 248 |
| Bootloader, default release | 14,732 | 14,732 | +0 | 32 |
| Codec probe, default release | 8,700 | 21,136 | +12,436 | 24 |
| Codec probe, `z` + LTO + 1 codegen unit | 5,380 | 6,480 | +1,100 | 24 |

The current application entry points do not call these codecs. Their sizes therefore
do not measure codec adoption cost; the four-byte backplane difference should not be
interpreted as a Deku saving. The separate `carrier/examples/codec-size.rs` probe
uses opaque inputs and consumes all four codec results, keeping the relevant code
linked. It also retains the existing control dispatcher. It is a link-only probe,
not an application image to flash.

The probe links without an allocator. Static RAM does **not** include stack usage;
stack high-water marks and MCU execution time were not measured. The optimized
profile was supplied only to probe build commands; repository release settings
were not changed. Its result is not an estimate for every future message layout.

## Host timing

AMD Ryzen 9 5950X, x86_64 Linux, Rust 1.98.1, default Cargo bench/release profile.
Values are median nanoseconds per full public codec call. Each process warms up
for 100,000 calls, then measures 21 batches of 100,000 calls per operation. The
table takes the median of three process medians, alternating variant order.
There are 256 varied inputs; half the policy inputs omit PD limits. Inputs and
outputs pass through `black_box`. Timing includes indexing/loop overhead; CPU
affinity and frequency were not fixed. These are host measurements, not MCU timing.

| Operation | Manual ns | Deku ns | Deku/manual |
|---|---:|---:|---:|
| `encode_power` | 8.23 | 8.14 | 0.99× |
| `decode_power` | 10.61 | 9.50 | 0.90× |
| `encode_policy` | 22.26 | 31.77 | 1.43× |
| `decode_policy` | 13.78 | 14.81 | 1.07× |
| `reject_power_reserved` | 7.60 | 7.60 | 1.00× |
| `reject_policy_reserved` | 11.17 | 11.17 | 1.00× |

The largest observed host regression is policy encoding. Power decoding is faster
in this host build; small differences in the other operations should not drive an
embedded design decision. Rejected reserved bytes are checked before the Deku
parser runs, so similar rejection timings are expected.

## Reproduction

From the repository root, create a separate checkout of the original base and
apply the archived trial patch. This reproduces the experiment without changing
the firmware on `main`:

```sh
git worktree add --detach ../modular-rack-power-deku-trial \
  9680cd3992865a311ab963fa6192fc00b31434e8
git -C ../modular-rack-power-deku-trial apply \
  "$PWD/firmware/docs/pdcan/deku-trial.patch"
cd ../modular-rack-power-deku-trial
rustup target add thumbv6m-none-eabi --toolchain 1.98.1
rustup component add llvm-tools-preview --toolchain 1.98.1
python3 firmware/scripts/bench_deku.py
```

The runner reads the pinned version from `firmware/rust-toolchain.toml`, passes it
explicitly to Rust commands, uses `--locked`, and keeps separate build directories
for the two variants. It runs protocol tests, compares complete corpus output,
builds all three firmware images and both probe profiles, captures feature trees,
and performs the host timing runs. A corpus mismatch fails the run and saves both
outputs for inspection. Results are written to `firmware/target/deku-bench/results.json`.

This run is preserved in [deku-benchmark-results.json](deku-benchmark-results.json),
including per-process timing ranges and feature trees.

In the separate checkout prepared above, the rejected experiment can also be
built directly (this is not a production build configuration):

```sh
cd firmware
cargo +1.98.1 build -p carrier-firmware --bin carrier --release \
  --target thumbv6m-none-eabi --features firmware-bin,pdcan-protocol/deku-codec
```

Broader adoption is not planned. Reconsideration requires a new decision supported
by materially different requirements or measurements under the intended firmware
release profile, including MCU latency and stack usage. The current decision is
to retain the manual firmware codecs.
