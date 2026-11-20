#!/bin/sh
# Stamp the standalone adapter the same way as the main board releases.
set -eu
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 OUTPUT_DIRECTORY" >&2
    exit 2
fi

board_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
revision=$(git -C "$board_dir" rev-parse --verify 'HEAD^{commit}')
build_date=$(TZ=UTC git -C "$board_dir" show -s --format=%cd --date=format-local:%y.%m.%d "$revision")
short_hash=$(printf '%.12s' "$revision")
exec kicad-cli pcb export gerbers -o "$1" \
    -D "BUILD_DATE=$build_date" -D "SHORT_HASH=$short_hash" \
    -l F.Cu,B.Cu,F.Mask,B.Mask,F.Silkscreen,B.Silkscreen,Edge.Cuts \
    "$board_dir/jlink-swd.kicad_pcb"
