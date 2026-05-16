#!/usr/bin/env bash
# Dogfood rprof against a `cargo build` of its own source.
#
# Usage:
#   scripts/dogfood.sh                # build → capture → view (default: `all`)
#   scripts/dogfood.sh build          # build rprof into dogfood/bin/rprof
#   scripts/dogfood.sh capture        # build, then capture release + debug profiles
#   scripts/dogfood.sh view           # build, then re-render dogfood/report.html
#   scripts/dogfood.sh all            # explicit form of the default
#
# The fast inner loop for viewer changes is `view`: it rebuilds rprof
# incrementally (reusing the dev `target/` cache) and re-renders the HTML
# from the cached JSON, without paying the cost of another full capture.

set -euo pipefail

cmd="${1:-all}"

project_root=$(cd "$(dirname "$0")/.." && pwd)
dogfood="$project_root/dogfood"
bin="$dogfood/bin/rprof"
release_json="$dogfood/release.json"
debug_json="$dogfood/debug.json"
report_html="$dogfood/report.html"

log() { printf '\n>>> %s\n' "$*"; }

build_rprof() {
    log "Building rprof (release)"
    (cd "$project_root" && cargo build --release)
    mkdir -p "$dogfood/bin"
    cp "$project_root/target/release/rprof" "$bin"
}

# Stage a fresh copy of the rprof source under a workload directory so the
# cargo build we profile has no cache to fall back on.
stage_workload() {
    local work=$1
    rm -rf "$work"
    mkdir -p "$work"
    # Copy the inputs `cargo build` actually needs. `tests/` is only consulted
    # by `cargo test`, so we skip it to keep the workload focused.
    cp -R "$project_root/src" "$work/src"
    cp -R "$project_root/assets" "$work/assets"
    cp "$project_root/Cargo.toml" "$work/Cargo.toml"
    cp "$project_root/Cargo.lock" "$work/Cargo.lock"
    cp "$project_root/README.md" "$work/README.md"
    cp "$project_root/LICENSE" "$work/LICENSE"
}

capture_one() {
    local profile=$1 out_json=$2
    local work="$dogfood/workload-$profile"
    log "Staging workload for $profile build → $work"
    stage_workload "$work"
    log "Capturing $profile build → $out_json"
    local args=(run -o "$out_json" --)
    if [[ "$profile" == "release" ]]; then
        args+=(cargo build --release)
    else
        args+=(cargo build)
    fi
    (cd "$work" && "$bin" "${args[@]}")
}

capture() {
    capture_one release "$release_json"
    capture_one debug "$debug_json"
}

view() {
    if [[ ! -f "$release_json" || ! -f "$debug_json" ]]; then
        echo "error: missing captured JSON under $dogfood/. Run \`$0 capture\` first." >&2
        exit 1
    fi
    log "Rendering viewer → $report_html"
    "$bin" view --no-open -o "$report_html" \
        --label "release:$release_json" \
        --label "debug:$debug_json"
    cat <<EOF

Report ready: $report_html
  Open in VS Code:   code "$report_html"
  Or Ctrl+Click the path above (most terminals will open it).
EOF
}

case "$cmd" in
    build)
        build_rprof
        ;;
    capture)
        build_rprof
        capture
        ;;
    view)
        build_rprof
        view
        ;;
    all|"")
        build_rprof
        capture
        view
        ;;
    -h|--help|help)
        sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
        ;;
    *)
        echo "error: unknown command \`$cmd\`. Run \`$0 --help\`." >&2
        exit 2
        ;;
esac
