# Carrier and backplane fabrication panel

Build the complete release, including its top-side PNG render, with:

```sh
mise run panel:build
```

This also generates three top-side documentation PNGs and a provenance manifest
under `docs/assets/generated/pcbs/`:

- `carrier-rev-a-top.png`
- `backplane-prototype-rev-a-top.png`
- `carrier6-backplane1-rev-a-top.png`
- `renders.json`

To rebuild the PNGs without exporting the fabrication bundles, run:

```sh
mise run docs:pcb-renders
```

Generate the carrier and backplane InteractiveHtmlBom pages with:

```sh
mise run docs:pcb-iboms
```

This task first runs `docs:pcb-renders`, ensuring that the iBOMs use the same
Git revision as the PNGs and panel. It currently uses the official
KiBot container because KiBot is not installed on the host and the host's
KiCad 10/Python 3.14 bindings require a `SwigPyIterator` compatibility patch
when InteractiveHtmlBom runs directly. The task contains a note to revisit the
container once the host toolchain no longer needs that workaround. Podman is
required, and the first run pulls the pinned image. `KIBOT_IMAGE` can override
the default image for testing.

All three commands are file-based mise tasks under `.mise/tasks/`, so their
multi-command implementations do not add noise to `mise.toml`.

## Documentation renders

The individual images are rendered from the committed board files at the build
revision, not from uncommitted PCB files in the worktree. `renders.json` records
that revision, the panel build hash, KiCad version, and actual output dimensions.

The iBOM configuration marks footprints carrying KiCad's native DNP flag as not
fitted. The generated assembly tables therefore match the released BOMs: 57
carrier references and 67 backplane references. Manufacturer, MPN, and LCSC
fields are included alongside value and footprint.

KiBot reports expected warnings about the container having no personal KiCad
configuration, an unreferenced `REF**` board graphic, and backplane mounting
holes `H7`/`H8` being matched by reference rather than UUID. These do not alter
the generated fitted-reference counts.

### Carrier Rev A

![Carrier Rev A top-side render](assets/generated/pcbs/carrier-rev-a-top.png)

[Open the carrier InteractiveHtmlBom](assets/generated/pcbs/carrier-rev-a-ibom.html)

### Backplane prototype Rev A

![Backplane prototype Rev A top-side render](assets/generated/pcbs/backplane-prototype-rev-a-top.png)

[Open the backplane prototype InteractiveHtmlBom](assets/generated/pcbs/backplane-prototype-rev-a-ibom.html)

### Six-carrier/one-backplane panel

![Six-carrier/one-backplane panel top-side render](assets/generated/pcbs/carrier6-backplane1-rev-a-top.png)

The Rev A customer panel combines six carrier PCBs and one backplane-prototype
PCB into one fabrication and top-side assembly unit. The generated result is
163.800 x 237.350 mm, so it fits both vendors considered:

- JLCPCB recommends keeping an assembly panel at or below 250 x 250 mm.
- PCBWay states an assembly-panel range of 50 x 50 mm through 330 x 530 mm.

The source boards have different copper weights. The common panel therefore
uses the heavier backplane construction: four layers, 1.6 mm FR-4, ENIG,
2 oz outer copper, and 1 oz inner copper. Both vendors advertise this copper
combination, but it must be confirmed during engineering review before payment.

## Source authority

The builder materializes both boards and both BOMs from one Git revision. The
mise tasks set that revision to the checked-out `HEAD`; a deployment can set
`PANEL_GIT_HASH` to another commit available in the checkout. This keeps the
panel sources, board identity text, documentation renders, and provenance
metadata on the same commit without source hashes embedded in the script.

KiKit applies a unique prefix to every reference and net. The final assembly
data contains 409 placed parts: 342 carrier placements and 67 backplane
placements.

The panel, both Gerber exports, and the position file use the same absolute
origin. A 1 mm positive coordinate margin keeps plotted edge strokes away from
negative Gerber coordinates while preserving origin agreement between KiKit's
JLCPCB and PCBWay exporters.

The mise task passes the current Git short hash through `PANEL_GIT_HASH`. The
generator validates it, uses it to materialize the source files, and places
`PANEL <hash>` on the top process rail on `F.SilkS`. Baking the resolved value
into the generated PCB is intentional:
KiCad supports `${GIT_HASH}` project text variables and `kicad-cli -D`
overrides, but KiKit's vendor fabrication commands do not expose that override.

Board identity text uses native KiCad project text variables. The editable
defaults live in each board's `.kicad_pro`; for example, the backplane uses
`PROJECT_FAMILY`, `BOARD_NAME`, `BOARD_VERSION`, `BUILD_DATE`, and
`SHORT_HASH`. Its working-copy defaults render as:

```text
mrp.backplane
v2.1-YY.MM.DD
UNRELEASED
```

For a variable-bearing board, the panel builder deterministically replaces
`BUILD_DATE` with the build revision's commit date and `SHORT_HASH` with that
revision. It bakes those values before
KiKit copies the board, so the standalone renders, panel PCB, and Gerbers all
agree. Use `-D KEY=VALUE` for a shared override, or
`--carrier-define-var KEY=VALUE` / `--backplane-define-var KEY=VALUE` for a
board-specific override. Explicit overrides take precedence over both project
defaults and the automatically derived date/hash.

## Mechanical design

The backplane is rotated 90 degrees at the left of a vertical column of six
carriers. Routed separation is at least 2 mm. Five-millimetre breakaway tabs use
0.6 mm non-plated mouse-bite holes on 0.95 mm pitch, leaving 0.35 mm between
holes. The perimeter rails carry four 2 mm non-plated tooling holes and three
1 mm copper fiducials with 2 mm mask openings.

The carrier release includes two non-plated holes attached to the DNP `MOD1`
footprint outside its nominal board outline. The wider horizontal panel gap is
deliberate: it leaves those holes in routed waste instead of placing them in a
neighbouring PCB or rail.

## Vendor setup

For either vendor, order the supplied customer panel without vendor
re-panelization and declare two different designs. Order quantity is the number
of complete panels, not the number of individual daughterboards. Upload the
nested Gerber ZIP for PCB fabrication, then the vendor-specific `BOM.csv` and
`positions.csv` for top-side assembly.

For JLCPCB, select the complete panel BOM/CPL workflow and prefer Standard PCBA
so the non-default copper construction receives engineering review. For PCBWay,
submit `positions.csv` as the centroid/pick-and-place file. Some source BOM
entries do not include an explicit manufacturer and MPN. The PCBWay BOM retains
the released LCSC part number and flags these rows for confirmation rather than
inventing sourcing data.

Vendor references checked for this design:

- [JLCPCB PCB assembly capabilities](https://jlcpcb.com/capabilities/pcb-assembly-capabilities)
- [JLCPCB mouse-bite panelization guide](https://jlcpcb.com/blog/mouse-bite-panelization-guide)
- [JLCPCB process-edge and tooling-hole specification](https://jlcpcb.com/help/article/specifications-for-adding-process-edges-and-positioning-holes)
- [JLCPCB copper-weight capability](https://jlcpcb.com/help/article/jlcpcb-copper-weight)
- [JLCPCB different-design fee rules](https://jlcpcb.com/help/article/in-what-cases-will-there-be-charged-extra)
- [JLCPCB complete BOM/CPL upload guidance](https://jlcpcb.com/help/article/common-bom-and-cpl-matching-issues-and-explanations)
- [PCBWay assembly panel requirements](https://www.pcbway.com/pcb_prototype/Panel_Requirements_for_Assembly.html)
- [PCBWay rails, fiducials, and tooling holes](https://www.pcbway.com/helpcenter/design_instruction/PCB_Panelization__Breakaway_Rails__Fiducial_Marks__Tooling_Holes.html)
- [PCBWay assembly-file requirements](https://www.pcbway.com/helpcenter/pcb_assembly_ordering/What_files_are_requested_for_assembly_production_.html)
- [PCBWay fabrication tolerances](https://www.pcbway.com/pcb_prototype/PCB_Manufacturing_tolerances.html)

## Validation

The build checks for all four copper layers, masks, top silkscreen, edge cuts,
and plated/non-plated drill files in both Gerber archives. It also verifies 409
positions, 135 mouse-bite holes, four tooling holes, and three fiducials.

KiCad DRC reports zero unconnected items and no copper-clearance, drill-
clearance, or outline failures. It still reports inherited release-board
courtyard, silkscreen, and library warnings/errors. The panel-created DRC items
are mouse-bite holes inside the courtyard of DNP `MOD1`; they do not intersect
copper or another drill. `validation.json` records the exact generated counts.
