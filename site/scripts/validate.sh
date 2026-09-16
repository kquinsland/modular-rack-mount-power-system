#!/usr/bin/env bash
set -euo pipefail

failure=0
check_outputs=true
if [[ "${1:-}" == "--source-only" ]]; then
  check_outputs=false
elif [[ -n "${1:-}" ]]; then
  echo "usage: site/scripts/validate.sh [--source-only]" >&2
  exit 2
fi

while IFS= read -r image; do
  echo "generated/publication raster must be WebP: $image" >&2
  failure=1
done < <(find site/content docs/assets/generated/pcbs -type f \( -iname '*.png' -o -iname '*.jpg' -o -iname '*.jpeg' \))

while IFS= read -r -d '' worklog; do
  filename="${worklog##*/}"
  if [[ ! "$filename" =~ ^wl\.([0-9]{4}-[0-9]{2}-[0-9]{2})\ -\ [A-Za-z0-9][A-Za-z0-9._+\ -]*\.md$ ]]; then
    echo "invalid worklog filename: $worklog" >&2
    failure=1
    continue
  fi

  filename_date="${BASH_REMATCH[1]}"
  frontmatter_date="$(sed -nE "s/^date:[[:space:]]*['\"]?([0-9]{4}-[0-9]{2}-[0-9]{2}).*/\\1/p" "$worklog")"
  if [[ "$frontmatter_date" != "$filename_date" ]]; then
    echo "worklog date does not match filename: $worklog" >&2
    failure=1
  fi
done < <(find site/content/worklogs -maxdepth 1 -type f -name '*.md' ! -name '_index.md' -print0)

if (( failure != 0 )); then
  exit 1
fi

if [[ "$check_outputs" == true ]]; then
  required_outputs=(
    site/public/index.html
    site/public/index.md
    site/public/llms.txt
    site/public/system/index.html
    site/public/system/index.md
    site/public/guides/index.html
    site/public/guides/index.md
    site/public/worklogs/index.html
    site/public/worklogs/index.md
    site/public/system/hardware/_files/panel.webp
    site/public/system/hardware/backplane/_files/renders.json
    site/public/system/hardware/modules/_files/carrier.webp
    site/public/guides/assembly/_files/backplane-ibom.html
  )
  for output in "${required_outputs[@]}"; do
    if [[ ! -f "$output" ]]; then
      echo "missing generated output: $output" >&2
      failure=1
    fi
  done

  if ! grep -q 'mrp.karlquinsland.com' site/public/llms.txt; then
    echo "llms.txt does not contain the canonical domain" >&2
    failure=1
  fi

  if ! grep -Eq 'rel=("alternate"|alternate)' site/public/system/index.html; then
    echo "technical page does not advertise an alternate representation" >&2
    failure=1
  fi

  if ! grep -q 'View source on GitHub' site/public/system/index.html; then
    echo "technical page does not include its source link" >&2
    failure=1
  fi

  for obsolete in site/public/latest site/public/worklog; do
    if [[ -e "$obsolete" ]]; then
      echo "obsolete documentation output exists: $obsolete" >&2
      failure=1
    fi
  done
fi

exit "$failure"
