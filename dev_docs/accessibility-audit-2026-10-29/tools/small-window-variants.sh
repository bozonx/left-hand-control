#!/usr/bin/env bash
# Runs small-window variants inside the private compositor started by run-isolated.sh.
set -u
out=$LHC_AUDIT_OUT
bin=$LHC_AUDIT_ROOT/target/debug/examples/small-window
while read -r name lang size release; do
  mkdir -p "$out/$name"
  env LHC_SMALL_LANGUAGE="$lang" LHC_WINDOW_SIZE="$size" ${release:+LHC_SMALL_RELEASE=1} LHC_EDITOR_SNAPSHOTS="$out/$name" "$bin" > "$out/$name/log.txt" 2>&1
  echo "$name exit=$?" >> "$out/summary.txt"
done < "$LHC_VARIANTS"
