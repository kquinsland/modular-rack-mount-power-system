# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "opencv-python-headless", "pillow", "cairosvg"]
# ///
"""Trace supplied white artwork to filled SVG paths; never writes KiCad files.

Run from any directory with: uv run hardware/artwork/indy/trace.py
"""

from pathlib import Path
import xml.etree.ElementTree as ET

import cairosvg
import cv2
import numpy as np
from PIL import Image


OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
NS = "http://www.w3.org/2000/svg"
ET.register_namespace("", NS)


def trace(source: str, name: str):
    rgb = np.asarray(Image.open(ROOT / source).convert("RGB"))
    # White ink has all channels above 150; green soldermask is background.
    mask = (rgb.min(axis=2) > 150).astype(np.uint8)
    ys, xs = np.where(mask)
    bounds = (xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)
    x0, y0, x1, y1 = map(int, bounds)
    mask = mask[y0:y1, x0:x1]
    height, width = mask.shape
    # Upscale the binary field before contouring so the pixel boundary, rather
    # than the original pixel centers, determines the contour dimensions.
    enlarged = cv2.resize(mask, None, fx=4, fy=4, interpolation=cv2.INTER_NEAREST)
    contours, _ = cv2.findContours(enlarged, cv2.RETR_TREE, cv2.CHAIN_APPROX_SIMPLE)
    paths = []
    for contour in contours:
        points = cv2.approxPolyDP(contour, 1.0, True).reshape(-1, 2) / 4
        if len(points) < 3:
            continue
        paths.append("M" + " L".join(f"{x:.2f},{y:.2f}" for x, y in points) + " Z")
    data = " ".join(paths)
    svg = ET.Element(f"{{{NS}}}svg", {
        "width": f"{75 * width / height:.6f}mm", "height": "75mm",
        "viewBox": f"0 0 {width} {height}",
    })
    ET.SubElement(svg, f"{{{NS}}}title").text = f"Indy silkscreen traced from {source}"
    ET.SubElement(svg, f"{{{NS}}}desc").text = (
        "Black filled paths represent white silkscreen ink. Background and holes "
        "are transparent. Nominal artwork height is 75 mm. Fine details have not "
        "been thickened or certified against a fabricator's minimum width."
    )
    ET.SubElement(svg, f"{{{NS}}}path", {
        "d": data, "fill": "#000000", "fill-rule": "evenodd", "stroke": "none",
    })
    target = OUT / f"{name}-75mm.svg"
    ET.ElementTree(svg).write(target, encoding="utf-8", xml_declaration=True)
    # Render back and compare with the classified source to catch polarity,
    # lost holes, or contour errors. This is a fidelity check, not a DFM check.
    png = cairosvg.svg2png(url=str(target), output_width=width, output_height=height)
    from io import BytesIO
    rendered = np.asarray(Image.open(BytesIO(png)).convert("RGBA"))[:, :, 3] >= 128
    intersection = np.count_nonzero(rendered & (mask != 0))
    union = np.count_nonzero(rendered | (mask != 0))
    iou = intersection / union
    if iou < 0.97:
        raise RuntimeError(f"Trace fidelity too low: {iou:.4f}")
    print(f"{target.name}: {75 * width / height:.3f} x 75 mm; {len(paths)} contours; mask IoU {iou:.4f}")
    return data, width, height


def proof(data: str, width: int, height: int):
    svg = ET.Element(f"{{{NS}}}svg", {
        "width": "210mm", "height": "297mm", "viewBox": "0 0 210 297",
    })
    ET.SubElement(svg, f"{{{NS}}}rect", {"width": "210", "height": "297", "fill": "white"})

    def text(x, y, value, size=3.5):
        ET.SubElement(svg, f"{{{NS}}}text", {
            "x": str(x), "y": str(y), "font-family": "sans-serif", "font-size": str(size),
        }).text = value

    text(12, 14, "Indy: actual-size silkscreen comparison", 5)
    text(12, 22, "Print at 100% / Actual Size. Do not fit to page.")
    text(12, 28, "White = silk ink; green = board. Screen size is not calibrated.")
    for x, y, size in [(15, 42, 50), (100, 42, 75), (15, 140, 100)]:
        scale = size / height
        text(x, y - 5, f"{size} mm tall x {width * scale:.1f} mm wide")
        ET.SubElement(svg, f"{{{NS}}}rect", {
            "x": str(x - 2), "y": str(y - 2), "width": str(width * scale + 4),
            "height": str(size + 4), "fill": "#005b32",
        })
        ET.SubElement(svg, f"{{{NS}}}path", {
            "d": data, "transform": f"translate({x} {y}) scale({scale})",
            "fill": "white", "fill-rule": "evenodd", "stroke": "none",
        })
    text(110, 156, "75 mm: preferred starting size")
    text(110, 163, "100 mm: more fine detail")
    text(110, 170, "50 mm: fine lines may drop out")
    text(12, 260, "Artwork is a faithful trace, not a fabrication simulation.")
    text(12, 267, "Whiskers, eye details and fur tips still need fabrication review.")
    ET.SubElement(svg, f"{{{NS}}}path", {
        "d": "M12 279 H62 M12 277 V281 M62 277 V281", "fill": "none",
        "stroke": "black", "stroke-width": "0.25",
    })
    text(12, 287, "This bar must measure 50 mm on paper.")
    target = OUT / "indy-size-proof.svg"
    ET.ElementTree(svg).write(target, encoding="utf-8", xml_declaration=True)
    cairosvg.svg2pdf(url=str(target), write_to=str(OUT / "indy-size-proof.pdf"))
    cairosvg.svg2png(url=str(target), write_to=str(OUT / "indy-size-proof.png"), output_width=1050)


if __name__ == "__main__":
    primary = trace("indy04.png", "indy04-silkscreen")
    trace("indy02.png", "indy02-silkscreen")
    proof(*primary)
