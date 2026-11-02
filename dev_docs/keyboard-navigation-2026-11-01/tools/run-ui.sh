#!/usr/bin/env bash
set -eu
export LHC_EDITOR_SNAPSHOTS="$LHC_AUDIT_OUT"
: > "$LHC_AUDIT_OUT/results.txt"
for scenario in editor interactions keyboard-navigation popup-grid layer-keys menus library small-window macros inline-edit undo-redo drag-scroll notifications; do
  if [ "$scenario" = editor ]; then
    timeout 90 target/debug/examples/editor --smoke > "$LHC_AUDIT_OUT/$scenario.log" 2>&1
  else
    timeout 90 "target/debug/examples/$scenario" > "$LHC_AUDIT_OUT/$scenario.log" 2>&1
  fi
  printf '%s PASS\n' "$scenario" >> "$LHC_AUDIT_OUT/results.txt"
done
