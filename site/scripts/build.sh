#!/usr/bin/env bash
set -euo pipefail

site/scripts/validate.sh --source-only
hugo --source site --minify --gc --cleanDestinationDir
site/scripts/validate.sh
