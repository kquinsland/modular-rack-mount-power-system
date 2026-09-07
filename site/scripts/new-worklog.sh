#!/usr/bin/env bash
set -euo pipefail

summary="${*:-}"
if [[ -z "$summary" ]]; then
  echo 'usage: mise run worklog:new -- "Concise Summary"' >&2
  exit 2
fi

if [[ "$summary" == *$'\n'* || "$summary" == */* || "$summary" == *\\* ]]; then
  echo "summary must be one line and cannot contain path separators" >&2
  exit 2
fi

entry_date="$(date +%F)"
relative_path="worklog/wl.${entry_date} - ${summary}.md"
absolute_path="site/content/${relative_path}"

if [[ -e "$absolute_path" ]]; then
  echo "worklog already exists: $absolute_path" >&2
  exit 1
fi

hugo new content --source site --kind worklog "$relative_path"
echo "created $absolute_path"
