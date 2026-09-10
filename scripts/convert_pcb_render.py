#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["Pillow==12.1.0"]
# ///
"""Convert a still PCB render to lossless WebP, preserving exact RGBA pixels."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path

from PIL import Image, features


def convert(source: Path, destination: Path) -> dict:
    if destination.suffix.lower() != ".webp":
        raise ValueError("The destination must end in .webp")
    if source.resolve() == destination.resolve():
        raise ValueError("Source and destination must be different files")
    with Image.open(source) as original:
        if getattr(original, "n_frames", 1) != 1:
            raise ValueError("PCB renders must be still images")
        # Create a fresh image so EXIF, ICC, XMP and other metadata do not leak.
        pixels = original.convert("RGBA")
        clean = Image.frombytes("RGBA", pixels.size, pixels.tobytes())
    destination.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=".webp-", dir=destination.parent)
    os.close(fd)
    temporary = Path(name)
    try:
        clean.save(temporary, format="WEBP", lossless=True, exact=True, method=6)
        with Image.open(temporary) as check:
            check.load()
            if check.format != "WEBP" or check.size != clean.size:
                raise RuntimeError("WebP format/dimensions did not survive encoding")
            if check.convert("RGBA").tobytes() != clean.tobytes():
                raise RuntimeError("Lossless conversion changed image pixels")
        digest = hashlib.sha256(temporary.read_bytes()).hexdigest()
        size = temporary.stat().st_size
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)
    return {
        "width": clean.width,
        "height": clean.height,
        "alpha": clean.getextrema()[3][0] < 255,
        "sha256": digest,
        "bytes": size,
        "format": "webp",
        "lossless": True,
        "encoder": {"pillow": Image.__version__, "libwebp": features.version("webp")},
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    print(json.dumps(convert(args.source, args.destination), sort_keys=True))


if __name__ == "__main__":
    main()
