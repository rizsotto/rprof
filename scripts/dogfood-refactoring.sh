#!/bin/sh
# compare-streaming.sh — dogfood rprof against itself, comparing the
# in-memory and streaming capture strategies in one combined report.
#
# Strategy: HEAD's rprof always plays the OUTER role and writes both
# JSONL files in the current schema. The INNER rprof differs between the
# two captures (HEAD vs the parent of 5f3b9ea, the streaming-refactor
# commit). What the outer captures is the inner's own resource usage,
# so the RSS chart visualises the difference directly:
#   * HEAD inner   — streams samples to disk each tick, RSS stays flat
#   * BEFORE inner — buffers all samples in memory, RSS grows monotonically
#
# Layout under ./compare-streaming/:
#   head/        shallow clone of master HEAD
#   before/      shallow clone at d815766 (parent of 5f3b9ea)
#   head.jsonl   capture of HEAD inner
#   before.jsonl capture of BEFORE inner
#   report.html  combined diff view

set -eu

PROJECT_ROOT=$(git rev-parse --show-toplevel)
WORK="$PROJECT_ROOT/dogfood"

# Resolve the short hash locally before handing it to clone: `git clone
# --revision` only accepts a full SHA or a ref name, not an abbreviation.
# dc0ed71 is the parent of d815766 — the streaming-refactor and its
# requirements file both landed on master from d815766 onward, so this
# is the last commit where capture still buffers samples in memory.
BEFORE_COMMIT=$(git rev-parse dc0ed71)

# Inner rprof sampling. Default is 100ms; with 1ms the BEFORE inner
# accumulates tens of thousands of RawSample entries over a cargo build,
# making the memory-vs-streaming difference dominate the RSS chart.
# The outer rprof keeps the 100ms default so its own chart isn't noisy.
INNER_INTERVAL=1ms
OUTER_INTERVAL=100ms

mkdir -p "$WORK"

log() { printf '\n>>> %s\n' "$*"; }

# Shallow clone of current master.
if [ ! -d "$WORK/head/.git" ]; then
    log "Cloning master HEAD → $WORK/head"
    git clone --depth 1 --branch master "file://$PROJECT_ROOT" "$WORK/head"
fi

# Shallow clone at a specific commit (git 2.49+).
if [ ! -d "$WORK/before/.git" ]; then
    log "Cloning $BEFORE_COMMIT → $WORK/before"
    git clone --revision="$BEFORE_COMMIT" --depth 1 \
        "file://$PROJECT_ROOT" "$WORK/before"
fi

log "Building rprof in $WORK/head"
(cd "$WORK/head" && cargo build --release --locked)

log "Building rprof in $WORK/before"
(cd "$WORK/before" && cargo build --release --locked)

HEAD_RPROF="$WORK/head/target/release/rprof"
BEFORE_RPROF="$WORK/before/target/release/rprof"
HEAD_JSONL="$WORK/head.jsonl"
BEFORE_JSONL="$WORK/before.jsonl"
REPORT_HTML="$WORK/report.html"

# Throwaway sink for the inner rprof's own report — only the OUTER
# capture is interesting. Both runners truncate on open, so reusing
# one path across the two captures is fine.
INNER_DISCARD="$WORK/inner-discard.out"

# --- Capture 1: HEAD rprof profiling HEAD rprof running cargo build --------
log "cargo clean (cold build for capture 1)"
(cd "$PROJECT_ROOT" && cargo clean)

log "Capturing HEAD-inner → $HEAD_JSONL"
(cd "$PROJECT_ROOT" && \
    "$HEAD_RPROF" run --interval "$OUTER_INTERVAL" -o "$HEAD_JSONL" -- \
    "$HEAD_RPROF" run --interval "$INNER_INTERVAL" -o "$INNER_DISCARD" -- \
    cargo build)

# --- Capture 2: HEAD rprof profiling BEFORE rprof running cargo build ------
log "cargo clean (cold build for capture 2)"
(cd "$PROJECT_ROOT" && cargo clean)

log "Capturing BEFORE-inner → $BEFORE_JSONL"
(cd "$PROJECT_ROOT" && \
    "$HEAD_RPROF" run --interval "$OUTER_INTERVAL" -o "$BEFORE_JSONL" -- \
    "$BEFORE_RPROF" run --interval "$INNER_INTERVAL" -o "$INNER_DISCARD" -- \
    cargo build)

# Both reports are HEAD JSONL — viewer can diff them.
log "Rendering diff report → $REPORT_HTML"
"$HEAD_RPROF" view --no-open -o "$REPORT_HTML" \
    --label "head:$HEAD_JSONL" \
    --label "before:$BEFORE_JSONL"

cat <<EOF

Done. Main repo unchanged (master HEAD); only target/ was cleaned.

Reports (both in HEAD's JSONL schema):
  $HEAD_JSONL    inner = HEAD rprof (streaming)
  $BEFORE_JSONL  inner = $BEFORE_COMMIT rprof (in-memory)

Combined view:
  $REPORT_HTML

What to look for: the BEFORE trace's RSS grows monotonically with sample
count; the HEAD trace stays flat. That is the streaming refactor visible
on the chart.
EOF
