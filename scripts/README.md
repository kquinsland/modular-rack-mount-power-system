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

## PCB production exports

Generate either board's checked-in BOM, component-placement file, Gerber/drill
archive, STEP assembly, IPC-D-356 netlist, designator inventory, assembly-count report, and validation manifest with:

```sh
mise run production:carrier
mise run production:backplane
```

The task runs ERC and DRC with schematic-parity checking before publishing any
files. It requires the selected board's design inputs to be committed, verifies
that the BOM and placement references agree exactly, checks the required
four-layer CAM file set, and stages everything before publishing the complete
file set in `hardware/boards/<board>/production/<silkscreen-id>/`, for example
`hardware/boards/backplane/production/mrp.backplane-v2.1-26-09-10-abcdef1/`.
The ZIP, CSVs, IPC netlist, reports, STEP and validation manifest all go inside
that release directory. Re-exporting the same release replaces only that
directory; other releases and any legacy files in the production root remain
intact. The manifest records `release_id`. DRC warnings are retained in
`validation.json`; ERC findings, DRC errors, unconnected items, parity errors,
missing sourcing fields, or BOM/CPL disagreement stop the export and leave the
previous production directory intact.

`assembly-report.md` lists the four PCBWay quote fields per single board:
unique parts (distinct LCSC codes), SMD components, BGA/QFP-category components,
and through-hole components. It follows PCBWay's tooltip definitions: the fields
count parts, despite the site's “SMT Pads” image labels. BGA/QFP includes ICs with
more than 16 pins (including QFN/SOP) and other SMD parts with more than 10 pins;
it is a subset of SMD. Only exported BOM/CPL references count, so hand-installed
connectors/modules excluded from those files do not enter the assembly quote.
The report includes a separate SMT contact count for reference. Repeated numbered
exposed-pad segments count once. Details are also stored under
`assembly.pcbway_quote` in `validation.json`. KiCad's Python bindings (`pcbnew`)
must be available to the task's Python interpreter. This report is published
alongside the fabrication ZIP and is covered by the validation manifest hash.

STEP assemblies are published alongside the ZIP as
`mrp.<board>-v<version>-YY-MM-DD-<hash>.step`, for example
`mrp.carrier-v2.1-26-09-10-abcdef1.step`. The project family, board name,
version, source-commit date and short hash come from the same release variables
as the silkscreen; filename dates use hyphens. STEP
exports use the center of the Edge.Cuts bounding box as the XY origin, cut via
holes in the board body, and include silkscreen and solder-mask faces. All
component categories are included, including DNP and unspecified footprints,
independently of BOM/CPL exclusions. VRML references use matching STEP/IGES models
where available. Footprints without assigned models cannot contribute component
geometry; their references and the export settings/origin are recorded under
`step` in `validation.json`. KiCad's model/geometry diagnostics are preserved in
the matching `<release-name>.step.log` as well as the task output. The exact STEP
filename is recorded in `step.file`. STEP text variables match the CAM release, and model paths resolve
relative to the original board project. STEP files are hashed in the manifest;
their internal exporter timestamps are not normalized.

The bottom silkscreen's `BUILD_DATE` and `SHORT_HASH` variables are baked from
the source commit. KiCad CAM timestamps are normalized to that same commit time
so every file carries consistent source provenance. For an isolated test
export, pass another output root after `--`; the release-ID subdirectory is
still appended:

```sh
mise run production:carrier -- --output-dir /tmp/carrier-production
mise run production:backplane -- --output-dir /tmp/backplane-production
```

## PCB release pipeline

The new KiBot/KiKit pipeline is described in the
[release tooling guide](../.kibot/release/README.md).

```sh
mise run release:setup
mise run release:check
mise run docs:pcb-renders
mise run docs:pcb-iboms
mise run release:build
```

`build_pcb_release.py` snapshots a committed hardware revision and stages the
pipeline configuration. `pcb_release_worker.py` runs checks and exports in a
digest-pinned container. `panel_layout.py` owns the six-carrier/one-backplane
geometry. `convert_pcb_render.py` is the PEP 723 lossless WebP converter, and
`validate_pcb_release.py` checks assembly and archive invariants.

Renders and per-board manifests publish into the Hugo content bundles under
`site/content/latest/hardware/`. Vendor proof WebPs stay outside fabrication
upload ZIPs. Preview mode never produces fabrication packages. Release mode
requires both source boards, the panel, and vendor variants to pass their gates.

`panel:build` invokes the new release builder and publishes to
`releases/pcb-release/`. The old `build_carrier_backplane_panel.py` and
standalone production exporter remain for historical comparison; existing Rev A
exports are not regenerated by this migration.

The architecture and Mermaid diagrams are in
[pcb-release-pipeline-refactor.md](../docs/pcb-release-pipeline-refactor.md).
