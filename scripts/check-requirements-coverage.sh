#!/bin/sh
# Check that every `status: implemented` requirement under `docs/requirements/`
# has at least one test tagged with `Requirements: <id>` under `src/` or
# `tests/`.
#
# Exits 0 if every implemented requirement is covered. Exits 1 (and prints
# the missing IDs) otherwise. Wired into CI's lint job; also runnable
# locally.
#
# Requirements at any other status (`proposed`, `accepted`, `planned`,
# `in-progress`, `deferred`, `rejected`) are skipped: they may not have
# tests yet, by design.
#
# POSIX sh; no bashisms.

set -eu

project_root=$(cd "$(dirname "$0")/.." && pwd)
cd "$project_root"

missing=
missing_count=0

for f in docs/requirements/*.md; do
    id=$(basename "$f" .md)
    [ "$id" = "CLAUDE" ] && continue
    status=$(awk '/^status:/{print $2; exit}' "$f")
    [ "$status" = "implemented" ] || continue
    if ! grep -rq "Requirements:.*\b${id}\b" src/ tests/; then
        missing="${missing}${id}
"
        missing_count=$((missing_count + 1))
    fi
done

if [ "$missing_count" -eq 0 ]; then
    echo "All implemented requirements have at least one tagged test."
    exit 0
fi

echo "error: implemented requirements with no tagged test:" >&2
printf '%s' "$missing" | while IFS= read -r id; do
    [ -n "$id" ] && echo "  - $id" >&2
done
echo >&2
echo "Add a 'Requirements: <id>' comment above a #[test] in src/ or tests/," >&2
echo "or change the requirement's status if it is not actually implemented." >&2
exit 1
