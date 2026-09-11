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
The mixed-panel validation policy also needs reconciliation before release use;
see the acceptance status below.

The host needs Python 3.11+, uv, and Podman. The container supplies KiBot 1.9.0,
KiCad 10.0.4, and KiKit 1.8.0-1. Builds use an offline container after setup.
Python packages for WebP are declared/pinned in `convert_pcb_render.py` using
PEP 723. No host KiCad Python installation or GUI is needed.

## Artifact boundaries

| Artifact | Destination |
| --- | --- |
| Carrier top/bottom | `site/content/system/hardware/modules/_files/carrier.webp`, `carrier-bottom.webp` |
| Backplane top/bottom | `site/content/system/hardware/backplane/_files/backplane.webp`, `backplane-bottom.webp` |
| Panel overview | `site/content/system/hardware/_files/panel.webp` |
| Render provenance | `renders.json` in each owning page's `_files/` directory |
| Optional interactive BOM | `site/static/guides/assembly/_files/carrier-ibom.html` / `backplane-ibom.html` |
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
vendors' standalone backplane CAM exports, proof rendering, twenty-one unit tests,
and Mermaid diagram rendering.

Panel compatibility fixes preserve explicit component-class reference lists
for each board instance, namespace `A.Reference`/`B.Reference` comparisons in
inherited rules, and bake only visible reference fields on their original
layers. Hidden fields must not become visible panel silkscreen. Component-class
filters other than explicit reference lists fail closed until supported; this
is not a general evaluator for all KiCad rule expressions or rule-area names.

The carrier source currently retains 27 `Under_PD_Module` members. Git history
shows the assignment list cleared in `b4fcf69`, `67bfaca`, and `8def932`, then
restored most recently in `361c928`. The commits record project saves but cannot
identify the process responsible; an editor saving stale in-memory settings is
a plausible explanation. The current panel problem was separate: KiKit did not
copy these top-level project assignments or rename the rule's `MOD1` reference.
The generated panel now has six assignments with 162 namespaced members; its
under-module overlaps and reference-specific courtyard exceptions pass DRC.

Full mixed-panel release acceptance is **not complete**:

- Carrier D1 now uses `mini-rack-power:D_SMC_Silk0.15mm` in both the schematic
  and PCB, retaining its pad geometry and fitted status. Native carrier checks
  after replacement report zero ERC/DRC errors and zero unconnected items;
  28 existing DRC warnings remain, with no D1 library-mismatch warning.
  The diagnostic counts below use an older source revision, not this PCB fix;
  repeat the mixed-panel check using the updated committed source.
- A diagnostic using source checkpoint `f12cc7b` and the compatibility fixes
  plus the mouse-bite policy below reports 215 panel errors: 126 repeated D1
  line-width assertions, 31 mouse-bite/courtyard findings,
  48 narrow GND connections, six visible carrier LED1 text-height findings,
  and four unconnected items. Under-module and reference-specific courtyard
  exceptions now work. These are generated-panel integration findings, not
  evidence of 215 errors in either source PCB. Reconcile the remaining policy
  and repeat the full checked-source/refill/panel sequence before accepting
  CAM equivalence. Do not blanket-ignore the findings.
- The 0.45 mm pad-hole spacing rule originates in `d28e3ec` and was copied to
  the carrier in `bedac27`. It also matches NPTH mouse bites, whose current
  0.6 mm holes have 0.4 mm edge-to-edge spacing. JLCPCB documents separate
  [mouse-bite guidance](https://jlcpcb.com/blog/pcb-design-efficiency-mouse-bites)
  from its general [pad-hole spacing limit](https://jlcpcb.com/capabilities/pcb-capabilities).
  The generated panel now appends a 0.30 mm hole-to-hole rule matching only
  **pairs of generated NPTH mouse bites** (`KiKit_MB_*`), eliminating all 108
  false generic pad-hole spacing findings in this diagnostic. Ordinary holes,
  tooling holes, mixed mouse-bite/ordinary-hole pairs, copper clearances, and
  component courtyards retain their existing checks. No source rule is changed.
  The generator validates actual 0.6 mm round unconnected NPTHs, 5–8 per set,
  0.30–0.40 mm edge gaps, and the expected 27 sets/135 holes. Metadata records
  the measured 1.0 mm pitch and 0.4 mm gap separately from KiKit's requested
  0.95 mm pitch. The capabilities table recommends 0.2–0.3 mm gaps while the
  more detailed mouse-bite guide recommends 0.35–0.4 mm (minimum 0.3 mm);
  this policy explicitly follows the latter. Confirm the panel with JLCPCB's
  engineering review when ordering rather than treating DRC as vendor approval.
- The panel inherits the backplane's 0.20 mm minimum copper connection width.
  The carrier leaves this check disabled (minimum 0.0 mm). Its In1/In2 GND
  zones contain 0.1651–0.1871 mm necks: widths of copper bridges, not copper
  thickness or clearances. Check the affected ground-current paths before
  widening the geometry or adopting a narrowly scoped alternate policy.
  Recommended next step: enable the same 0.20 mm connection check on the
  carrier, then address the eight findings per carrier (four on each inner
  layer) in the source layout. These zones already use solid pad connections
  and 0.25 mm minimum fill thickness, so this is not a thermal-spoke setting
  to turn up. Adjust nearby obstacles/connection geometry or remove unnecessary
  copper tongues only after checking alternate ground paths. Refill and verify
  connectivity before rebuilding. The common 0.20 mm policy is a design margin,
  not a claim that JLCPCB cannot fabricate any narrower copper.
- Vendor artwork still needs placement/registration as described above.
- Run both complete mixed-panel vendor branches and compare their CAM/assembly
  geometry once those gates pass. No fabrication-ready mixed-panel package has
  been produced by this implementation yet.

The old standalone production exporter and panel script remain available for
comparison while the migration is validated. The public `panel:build`,
`docs:pcb-renders`, and `docs:pcb-iboms` tasks use the new implementation.
The old Rev A production package remains a historical artifact.
