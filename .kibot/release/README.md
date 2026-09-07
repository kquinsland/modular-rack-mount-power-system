# PCB release tooling

The entry point is `scripts/build_pcb_release.py`, with file-based mise tasks.
It snapshots the selected commit, including the complete hardware library tree,
and runs KiBot/KiCad/KiKit inside the digest-pinned image defined by `IMAGE`.
Source projects in the checkout are never modified by a build.

Active projects are `hardware/boards/carrier/carrier.kicad_pro` and
`hardware/boards/backplane/backplane.kicad_pro`. The builder recognizes the
former `backplane-prototype` path in historical revisions and refuses to treat
the earlier split backplane as the consolidated board. Historical revisions
that used retired submodules need those pinned objects available locally.

```sh
mise run release:setup             # download the pinned image once
mise run release:test              # Python validation/conversion tests
mise run release:check             # both boards: ERC, parity, DRC, zone refill
mise run docs:pcb-renders          # unchecked preview + panel -> Hugo WebP bundles
mise run docs:pcb-iboms            # same, plus interactive assembly HTML
mise run release:build            # gated release -> build/pcb-release/
mise run panel:build              # gated release -> releases/pcb-release/ + Hugo
```

Pass `--ref COMMIT_OR_TAG` to select another committed source. The older
`PANEL_GIT_HASH` environment variable remains supported. Text-variable overrides
use `-D KEY=VALUE`, `--carrier-define KEY=VALUE`, or
`--backplane-define KEY=VALUE`. Board-specific overrides take precedence.
`BUILD_DATE` uses the source commit's UTC date; `SHORT_HASH` uses its first
12 hex digits. Every manifest records the full commit and effective variables.

For one board without changing site assets:

```sh
python scripts/build_pcb_release.py preview --board carrier --output build/carrier-preview
```

Preview is explicitly marked `preview-unchecked` and creates no fabrication
ZIPs. Release has no option to skip electrical checks. ERC findings, DRC errors,
unconnected items, and parity errors block it. Existing KiCad exclusions are
respected; active DRC/parity warnings are retained in validation summaries.
The current carrier's three DRC errors therefore prevent a complete release.
The mixed-panel validation policy also needs reconciliation before release use;
see the acceptance status below.

The host needs Python 3.11+, uv, and Podman. The container supplies KiBot 1.9.0,
KiCad 10.0.4, and KiKit 1.8.0-1. Builds use an offline container after setup.
Python packages for WebP are declared/pinned in `convert_pcb_render.py` using
PEP 723. No host KiCad Python installation or GUI is needed.

## Artifact boundaries

| Artifact | Destination |
| --- | --- |
| Carrier top/bottom | `site/content/latest/hardware/modules/carrier/carrier.webp`, `carrier-bottom.webp` |
| Backplane top/bottom | `site/content/latest/hardware/backplane/backplane.webp`, `backplane-bottom.webp` |
| Panel overview | `site/content/latest/hardware/panel.webp` |
| Render provenance | `renders.json` in each owning Hugo bundle |
| Optional interactive BOM | `site/static/assembly/carrier-ibom.html` / `backplane-ibom.html` |
| Manufacturing files | `<output>/<vendor>/upload/` and `<vendor>-upload.zip` |
| Vendor proof images | `<output>/<vendor>/proofs/<vendor>-{top,bottom}.webp` |
| Review metadata | `<output>/manifest.json`, `panel-info.json`, `checks/`, vendor order notes/netlist |
| Temporary source copies, PNGs and diagnostic logs | `build/pcb-release-work/<revision>-<unique>/` |

Upload ZIPs contain only `gerbers.zip`, `BOM.csv`, and `positions.csv`.
The Gerber archive has an explicit 13-file allowlist: four copper layers,
two masks, two silkscreens, two paste layers, the outline, and separate
PTH/NPTH Excellon drills. Proofs and order notes remain outside both archives.
The shared CPL uses millimetres and KiCad's Cartesian Y convention; negative
Y below the auxiliary origin is intentional. Rotation is normalized to 0–360°.
No automatic vendor-specific rotation correction is applied.

KiBot's manufacturer templates supply the CAM settings. Local overrides retain
both paste layers, use a shared origin, use metric drills, and explicitly
enable solder-mask subtraction for both vendors. PCBWay's upstream default has
mask subtraction disabled, so this override is deliberate.

## Vendor artwork setup

Create each conditional silk item as a board-only, padless footprint. Set these
custom footprint fields through the normal KiCad/Konnect editing workflow:

| Content | `ReleaseVendor` | `ReleaseArtwork` |
| --- | --- | --- |
| 8 × 8 mm JLC barcode target | `jlcpcb` | `barcode` |
| Literal `WayWayWay` job marker | `pcbway` | `job-number` |
| PCBWay logo | `pcbway` | `logo` |

Use a real unique reference, omit the footprint from BOM/positions, and keep
all artwork graphics on F.SilkS/B.SilkS. Pads, copper graphics, and 3D models
are rejected. The release requires the corresponding artwork roles for each
vendor; it fails closed if they have not been registered. The existing loose
barcode square must be moved into a tagged footprint, and the PCBWay marker
and logo still need to be supplied/placed before that gate can pass.

The container worker filters these board-only footprints on disposable panel
copies before KiBot exports. This small custom step handles graphics that
have no schematic component; ordinary BOM filters alone do not remove such
graphics reliably. It never guesses artwork from coordinates or deletes
untagged graphics. Both output variants receive another DRC check.

Proofs are PcbDraw views of the exact filtered PCB used for each export. They
verify artwork selection and placement; they are not a Gerber CAM simulation
of the fabricator's ink-clipping process. Inspect the actual plotted silkscreen
in GerbView as part of final manufacturing review.

## Publication and reproducibility

Checks and both vendor branches finish before publication. An existing output
directory must contain this tool's manifest to be replaced. Directory updates
use rename with rollback. Hugo publication replaces only known generated
files, preserves bundle content, and rolls back on an I/O failure. Publication
is serialized within this worktree; it is not a crash-atomic transaction across
separate filesystem paths.

CAM timestamps, ZIP ordering, file modes and ZIP timestamps are normalized.
The panel generator seeds KiCad's UUID generator from the commit. WebP encoding
is lossless and deterministic for the same input pixels. KiCad's ray-traced
images can differ slightly between runs: full release manifests, which include
image hashes, are therefore not promised byte-identical. They retain exact
source and tooling hashes for traceability.

## Acceptance status

Verified locally: both board previews, the mixed-panel preview, Hugo publication,
interactive BOMs, both boards' KiBot/native-KiCad BOM and LCSC agreement, both
vendors' standalone backplane CAM exports, proof rendering, ten unit tests,
and Mermaid diagram rendering.

Full mixed-panel release acceptance is **not complete**:

- The carrier source has three legend-line-width DRC errors at D1; the backplane
  passes the electrical/error gate, with warnings recorded.
- A diagnostic DRC on the preview panel reports 530 errors, including six
  unconnected items after refill. Inherited rules are not yet consistently
  scoped to their originating board: for example, a carrier legend rule
  applies to other carrier instances, and the backplane hole-spacing rule
  applies to panel mouse bites. Component-class exceptions and differing board
  defaults also need to survive panelization. These are generated-panel
  integration findings, not evidence of 530 errors in either source PCB.
  Reconcile this policy and repeat the full checked-source/refill/panel sequence
  before accepting CAM equivalence. Do not blanket-ignore the findings.
- Vendor artwork still needs placement/registration as described above.
- Run both complete mixed-panel vendor branches and compare their CAM/assembly
  geometry once those gates pass. No fabrication-ready mixed-panel package has
  been produced by this implementation yet.

The old standalone production exporter and panel script remain available for
comparison while the migration is validated. The public `panel:build`,
`docs:pcb-renders`, and `docs:pcb-iboms` tasks use the new implementation.
The old Rev A production package remains a historical artifact.
