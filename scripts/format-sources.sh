#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if (( $# > 1 )) || { (( $# == 1 )) && [[ "$1" != --check ]]; }; then
  printf 'Usage: bash scripts/format-sources.sh [--check]\n' >&2
  exit 2
fi
formatter=${AXONYX_FORMATTER:-$(git config --get axonyx.formatterPath || true)}
command=(cargo ax)
if [[ -n "$formatter" ]]; then command=("$formatter"); fi
mode=()
if (( $# == 1 )); then mode=(--check); fi
failed=0
count=0
while IFS= read -r -d '' file; do
  if ! "${command[@]}" fmt --file "$file" "${mode[@]}" > /dev/null; then
    printf 'Axonyx formatting failed: %s\n' "$file" >&2
    failed=1
  fi
  count=$((count + 1))
done < <(find app \( -path app/generated -o -path app/components/legura-editor \) -prune -o -type f \( -name '*.asx' -o -name '*.ax' \) -print0)
printf 'Processed %s hand-written Legura source files.\n' "$count"
exit "$failed"
