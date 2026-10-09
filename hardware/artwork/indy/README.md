# Indy silkscreen artwork

Faithful vector traces of the supplied dot-free artwork. No generative redraw,
line thickening, or removal of small details is applied. The selected source is
`indy04.png` (Image #3 in the supplied comparison); `indy02.png` (Image #1) is
provided as an alternative. Source PNGs are at the repository root.

## Files

- `indy04-silkscreen-75mm.svg`: preferred artwork, nominally 60.266 × 75 mm.
- `indy02-silkscreen-75mm.svg`: alternative, nominally 61.793 × 75 mm.
- `indy-size-proof.pdf`: A4 comparison of the preferred artwork at 50, 75, and
  100 mm tall. Print at **Actual Size / 100%**, without fitting to the page.
  Check the 50 mm calibration bar with a ruler.
- `indy-size-proof.svg` and `.png`: comparison sources and screen preview.
- `trace.py`: repeatable conversion and rendering; run with `uv run
  hardware/artwork/indy/trace.py` from the repository root.

## Import into KiCad

In PCB Editor, use **File → Import → Graphics**, select a `silkscreen` SVG,
choose **F.SilkS**, and enable **Group imported items**. Import scale 1 gives
75 mm height; use 0.666667 for 50 mm or 1.333333 for 100 mm. For back-side
artwork, check orientation as viewed from the back of the board.

Black SVG paths mean **silkscreen ink**; the board's fabrication ink color
determines their printed color. The background and internal openings are
transparent, with no background rectangle. Do not import the comparison sheet:
its colored rectangles and labels are only for viewing.

[KiCad's vector import documentation](https://docs.kicad.org/10.0/en/pcbnew/pcbnew.html#_importing_vector_graphics)
describes scale, layer, and grouping settings.

## Size and print fidelity

For preserving detail, prefer **100 mm tall** if space permits; **75 mm** is a
reasonable compromise. At **50 mm**, expect fine whiskers and facial details to
drop out. These are estimates, not a guaranteed manufacturing minimum.

The preferred source's cropped artwork is 998 × 1242 pixels. Sampled whisker
cross-sections include 2–4 pixel widths, equivalent to 0.081–0.161 mm at 50 mm
height, 0.121–0.242 mm at 75 mm, and 0.161–0.322 mm at 100 mm. These samples are
not an exhaustive minimum-width measurement; oblique cross-sections can also
overestimate perpendicular stroke width. Fur tips taper toward zero width at
every size. Larger printing preserves more detail but cannot preserve every
tip exactly.

[JLCPCB's published legend capability](https://jlcpcb.com/capabilities/pcb-)
lists a 0.15 mm minimum line width and 0.15 mm pad-to-silkscreen spacing.
Confirm the actual fabricator's requirements before production. The comparison
sheet shows ideal geometry, not a simulation of ink spreading or lost details.

The SVGs contain closed filled polygons with even-odd holes. The tracing script
classifies white source pixels, follows their boundaries with a 0.25 source-pixel
polygon approximation tolerance, and checks rendered mask overlap. Both traces
exceeded 99.99% intersection-over-union at source resolution. This checks visual
conversion fidelity, not manufacturability. No board placement, KiCad import
verification, or board DRC has been performed.
