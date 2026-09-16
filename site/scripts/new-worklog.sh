#!/usr/bin/env bash
set -euo pipefail

summary="${*:-}"
if [[ -z "$summary" ]]; then
  echo 'usage: mise run worklog:new -- "Concise Summary"' >&2
  exit 2
fi

if [[ ! "$summary" =~ ^[A-Za-z0-9][A-Za-z0-9._+\ -]*$ ]]; then
  echo "summary must start with a letter or number and contain only letters, numbers, spaces, dots, underscores, plus signs, or hyphens" >&2
  exit 2
fi

entry_date="$(date +%F)"
relative_path="worklogs/${entry_date:0:4}/${entry_date:5:2}/${entry_date:8:2} - ${summary}/index.md"
absolute_path="site/content/${relative_path}"

if [[ -e "$absolute_path" ]]; then
  echo "worklog already exists: $absolute_path" >&2
  exit 1
fi

hugo new content --source site --kind worklog "$relative_path"
mkdir -p "${absolute_path%/*}/_files"
echo "created $absolute_path"
