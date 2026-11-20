# Konnect-EDA

This repository uses [Konnect-EDA](https://github.com/Orchis-ai-LLC/Konnect-EDA), a Rust MCP
server, for AI-assisted KiCad work.

Mise downloads the precompiled upstream GitHub release. The version and release
artifact checksums are pinned in `mise.toml` and `mise.lock`:

```text
version: v0.14.1 (beta)
```

Konnect-EDA replaces `mixelpixx/Konnect`. It is commercial, closed-source
software, free for personal and business use during the beta. From 1.0,
upstream requires license keys and a commercial license for business use. See
the upstream [license](https://github.com/Orchis-ai-LLC/Konnect-EDA/blob/main/LICENSE)
and [licensing information](https://konnect-eda.orchis.ai/licensing/).

## Architecture

Konnect runs as one Rust binary. It uses KiCad 10's supported IPC API
for live PCB edits and direct, atomic S-expression editing for schematics.

## Setup

Install KiCad 10, then from the repository root run:

```shell
mise install
mise run konnect:verify
```

No Konnect compiler toolchain is required. Mise selects the precompiled GitHub
release for the host OS and architecture, verifies its pinned
checksum, and puts `konnect` on the project tool path.

Konnect-EDA v0.14.1 does not publish a standalone Linux ARM64 binary. The mise entry
therefore supports this repository's Linux x86-64 and macOS ARM64 targets and
deliberately excludes the generic Linux Plugin and Content Manager ZIP from
standalone asset matching. Other project tools remain available on Linux ARM64;
only Konnect is skipped.

Restart Codex after installation so it reloads `.codex/config.toml`. Useful checks:

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
standalone MCP server. Install the matching `konnect-pcm-v0.14.1-*` release if
you also want Konnect's KiCad GUI integration or native Specctra bridge.

## Codex skills

The six Konnect-EDA skills and their references live in `.codex/skills/`.
Codex discovers them through the repository's `.agents/skills` symlink.
A fresh checkout includes the skills without a user-level installation.

When upgrading Konnect-EDA, run `mise run konnect:install-guidance` to extract
the pinned binary's bundle into `~/.agents/skills`. This overwrites any user
edits to those installed skills. Check the bundle with
`mise exec -- konnect status --client codex`, then review and copy the six
updated directories into `.codex/skills/` together with their references.
