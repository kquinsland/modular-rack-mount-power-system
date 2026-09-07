# Scripts

Project automation goes here, such as KiCad CLI exports for schematics, BOMs, fabrication files, and 3D models.

## PCB power-capacity reports

`pcb_power_capacity.py` reads the filled copper geometry for a KiCad net and
estimates its current capacity at selected conductor temperature rises using
the IPC-2221 relationship published by KiCad. It reports every participating
copper layer's shared geometry, a natural current-sharing aggregate, an
ideal-sharing ceiling, voltage drop, copper loss, and a separate via-barrel
screening estimate. A scenario may compare multiple named copper stackups
without duplicating the geometry views. The generated SVG provides stackup
summary cards, color-coded limiting cuts, one conservative-temperature
capacity profile per stackup, and aggregate temperature-rise proxy heatmaps.
Each heatmap locally inverts the IPC-2221 capacity relationship; it is useful
for locating relative hot regions but is not a thermal-spreading or airflow
simulation.

The host needs Python 3.11 or newer, KiCad 10 with its `pcbnew` Python
bindings, and Shapely 2.x. The same dependencies are already used by the
repository's panel-generation workflow.

Generate both checked-in scenarios under `build/power-capacity/`:

```sh
mise run power:reports
```

Generate one named scenario or list the configured scenarios:

```sh
mise run power:report backplane-vcc
mise run power:report carrier-vcc
mise run power:report --list
```

Scenarios and their source/sink currents are defined in
`pcb_power_scenarios.toml`. The backplane scenario uses six 5.5 A slot loads;
it compares 2 oz outer / 1 oz inner copper with 1 oz outer / 0.5 oz inner
copper in one report. The carrier scenario follows `VCC` from `J1.1` to the
upstream side of the current shunt at `R1.1` and uses the board's KiCad
stackup. Reports show both micrometres and nominal PCB copper weight using
34.8 µm per oz/ft². Stackup comparisons use repeated `stackups` tables in the
scenario configuration. An ad-hoc invocation—or explicit `--copper` overrides
on a named scenario—generates a single custom-stackup report:

```sh
mise run power:report -- \
  --board hardware/boards/carrier/carrier.kicad_pcb \
  --net VCC \
  --source J1.1 \
  --sink R1.1=5.5 \
  --layer F.Cu \
  --layer In1.Cu \
  --layer In2.Cu \
  --layer B.Cu \
  --copper F.Cu=35 \
  --copper In1.Cu=17.5 \
  --copper In2.Cu=17.5 \
  --copper B.Cu=35
```

Omit `--layer` to use every enabled copper layer carrying the selected net.
Add one or more `--cut X1,Y1:X2,Y2` arguments to evaluate explicit board-space
cross-sections alongside the automatic sweep. A manual cut appears in the SVG
when it is limiting; every cut remains available in the complete JSON inventory.
The automatic sweep projects the source pad's actual polygon along the load
direction and places its first sample half a scan pitch beyond the pad edge, so
the source pad itself cannot be reported as a downstream copper bottleneck.

Each report directory contains:

- `report.md`: concise human-readable results and qualifications;
- `report.svg`: shared aggregate/per-layer geometry, stackup-specific limiting
  cuts and capacity profiles, and estimated temperature-rise proxy maps; and
- `report.json`: machine-readable inputs plus every stackup's scanned cuts,
  electrical estimates, limiting results, and peak temperature-rise proxy for
  future CI checks.

The all-scenarios task also writes `index.md` and `summary.json` at the report
root. The summary includes a board SHA-256 and an explicit pass/fail boolean
for each requested temperature rise, making it suitable for later CI policy
checks without scraping Markdown.

Run the formula and real-board integration tests with:

```sh
mise run power:test
```

This is a screening calculation, not a released board rating or a coupled
electro-thermal simulation. Review the SVG, heed extrapolation warnings, and
validate connectors, components, via topology, airflow, and first-article
temperatures separately.

## Carrier/backplane fabrication panel

`build_carrier_backplane_panel.py` uses KiKit to build one customer panel from
six carrier boards and one backplane-prototype board. It materializes both PCB
and BOM inputs from the build's Git revision, then emits separate JLCPCB and
PCBWay Gerber/BOM/positions bundles. The mise tasks use the checked-out `HEAD`;
`PANEL_GIT_HASH` may explicitly select another available commit.

```sh
mise run panel:build
```

The generated panel source and intermediate files are written under
`build/carrier-backplane-panel/`. The release directory also contains a
top-side PNG render for visual review. The full task also regenerates the
carrier, backplane, and combined-panel documentation PNGs under
`docs/assets/generated/pcbs/`. To rebuild only those documentation assets:

```sh
mise run docs:pcb-renders
```

Board identity silkscreen is sourced from the KiCad project's text variables.
For variable-bearing release sources, the builder replaces `BUILD_DATE` and
`SHORT_HASH` with the build revision's commit date and short hash before
KiKit copies the board. Direct script invocations may override variables with
`-D KEY=VALUE`, `--carrier-define-var KEY=VALUE`, or
`--backplane-define-var KEY=VALUE`.

Generate self-contained InteractiveHtmlBom assembly pages for the same build
revision with:

```sh
mise run docs:pcb-iboms
```

The iBOM task depends on the PNG task so both documentation formats use the
same staged source boards. It currently runs KiBot and InteractiveHtmlBom in a
container; the task itself documents why and what should be revisited before
moving those tools onto the host. Podman is required, and the first invocation
pulls the pinned container image. Set `KIBOT_IMAGE` to test another image.

These workflows are file-based mise tasks under `.mise/tasks/`, keeping the
multi-command implementation out of `mise.toml`. See
[`docs/carrier-backplane-panel.md`](../docs/carrier-backplane-panel.md) for the
layout, ordering constraints, and validation caveats.
