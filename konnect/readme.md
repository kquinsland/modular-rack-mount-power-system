# Konnect

This repository uses [Konnect](https://github.com/mixelpixx/Konnect), the Rust
successor to KiCAD-MCP-Server, for AI-assisted KiCad work through MCP.

Mise downloads the precompiled upstream GitHub release. The version and release
artifact checksums are pinned in `mise.toml` and `mise.lock`:

```text
version: v0.11.0
```

Konnect is currently beta software and is licensed AGPL-3.0-only. The previous
KiCAD-MCP-Server integration was MIT licensed. Review Konnect's `LICENSE` and
`COMMERCIAL.md` before using it in a commercial workflow.

## Why the integration changed

Konnect replaces the old Node, Python, third-party Python package, and KiCad
SWIG-binding stack with one Rust binary. It uses KiCad 10's supported IPC API
for live PCB edits and direct, atomic S-expression editing for schematics.

## Setup

Install KiCad 10, then from the repository root run:

```shell
mise install
mise run konnect:verify
```

No Konnect compiler toolchain is required. Mise selects the precompiled GitHub
release for the host OS and architecture, verifies its release provenance and
checksum, and puts `konnect` on the project tool path.

Konnect v0.11.0 does not publish a standalone Linux ARM64 binary. The mise entry
therefore supports this repository's Linux x86-64 and macOS ARM64 targets and
deliberately excludes the generic Linux Plugin and Content Manager ZIP from
standalone asset matching. Other project tools remain available on Linux ARM64;
only Konnect is skipped.

Restart Codex after the build so it reloads `.codex/config.toml`. Useful checks:

```shell
codex mcp list
codex mcp get konnect
```

The project MCP entry starts mise's `konnect` binary with the repository's
`konnect/config.toml`. It uses stdio transport, binds the default project
directory to the repository root, and initially exposes Konnect's small starter
toolset. Other toolsets are loaded on demand.

## KiCad runtime setup

In KiCad 10, enable **Preferences → Plugins → Enable KiCad API**. Most live PCB
operations require the target board to be open in PCB Editor. Schematic editing
and some closed-board operations do not require KiCad to be running.

On Linux, `kicad` and `kicad-cli` normally work from `PATH`. On macOS, add the
application paths to `konnect/config.toml` if they are not already discoverable:

```toml
kicad_cli = "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli"
kicad_binary = "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad"
ipc_address = "ipc:///tmp/kicad/api.sock"
```

The KiCad Plugin and Content Manager package is optional for this repository's
standalone MCP server. Install the matching `konnect-pcm-v0.11.0-*` release if
you also want Konnect's KiCad GUI integration or native Specctra bridge.

## Optional Codex guidance

Konnect bundles KiCad skills for Codex. Installing them writes outside this
repository to `~/.agents/skills`, so it is intentionally not part of the normal
build or verification tasks:

```shell
mise run konnect:install-guidance
konnect status --client codex
```

Remove those user-level files with:

```shell
konnect uninstall --client codex
```
