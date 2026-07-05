# KiCad MCP

Local setup notes for [mixelpixx/KiCAD-MCP-Server](https://github.com/mixelpixx/KiCAD-MCP-Server).

The upstream repo is checked out as a git submodule at:

```text
kicad-mcp/server
```

Current submodule revision:

```text
4f9828b
```

## Setup

From the project root:

```shell
git submodule update --init --recursive
mise install
mise run kicad-mcp:install
mise run kicad-mcp:verify
```

The `kicad-mcp:install` task does four things:

- Installs the Node dependencies with `npm ci`.
- Creates `kicad-mcp/server/.venv` with `uv`, using KiCad's bundled Python and `--system-site-packages`.
- Installs the Python requirements into that venv with `uv pip`.
- Builds the TypeScript server to `kicad-mcp/server/dist/index.js`.

Using a venv keeps third-party Python packages out of the KiCad app bundle while preserving access to `pcbnew`.

## Codex MCP Entry

The server is configured for this repo in `.codex/config.toml` as `kicad`.

Useful checks:

```shell
codex mcp list
codex mcp get kicad
```

The registered command is:

```shell
/opt/homebrew/bin/mise exec -C /Users/karl/Projects/mini-rack-power -- \
  node /Users/karl/Projects/mini-rack-power/kicad-mcp/server/dist/index.js
```

The local MCP environment includes:

- `LOG_LEVEL=info`
- `NODE_ENV=production`

The server itself finds `kicad-mcp/server/.venv/bin/python`, which was created from KiCad's bundled Python with `--system-site-packages`, so `pcbnew` stays importable without global MCP environment overrides.

## Upstream macOS Setup Script

The upstream `setup-macos.sh` script is a Claude Desktop config helper, not a full installer.

It:

- Supports `--verify`, `--dry-run`, and `--apply`.
- Resolves the repo root and expects `dist/index.js` to already exist.
- Checks `python3`, `node`, the built server artifact, and KiCad's bundled Python.
- Uses KiCad's Python to verify `import pcbnew`, read the Python version, and derive `site-packages`.
- Builds a Claude Desktop `mcpServers.<name>` JSON entry using `node dist/index.js`.
- Merges that entry into `~/Library/Application Support/Claude/claude_desktop_config.json`.
- Backs up the existing Claude config before writing and uses a temporary file for the final write.

It does not clone the repo, install Node dependencies, run the TypeScript build, or install Python requirements.

## Notes

`npm ci` currently reports upstream dependency audit findings: 6 vulnerabilities total, including 1 critical. A focused runtime audit with `npm audit --omit=dev` reports 1 high-severity advisory through `hono`. These are in the upstream dependency tree and were not auto-fixed here.
