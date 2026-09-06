# Scripts

Project automation goes here, such as KiCad CLI exports for schematics, BOMs, fabrication files, and 3D models.

## Carrier/backplane fabrication panel

`build_carrier_backplane_panel.py` uses KiKit to build one customer panel from
six released carrier boards and one released backplane-prototype board. It pins
the PCB and BOM inputs to the Git revisions matching their existing Rev A
fabrication releases, then emits separate JLCPCB and PCBWay Gerber/BOM/positions
bundles.

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

Generate self-contained InteractiveHtmlBom assembly pages for the same pinned
carrier and backplane revisions with:

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
