#!/usr/bin/env bash
# Run only inside run-isolated.sh; all product examples use disposable fixtures.
set -u
out=${LHC_AUDIT_OUT:?}
root=${LHC_AUDIT_ROOT:?}
mkdir -p "$out/scenarios"
export LHC_EDITOR_SNAPSHOTS="$out/scenarios"
: > "$out/scenario-results.tsv"
for name in editor interactions action-picker conditions library macros menus layer-keys inline-edit drag-scroll notifications undo-redo popup-grid; do
    export SNAP="$out/scenarios/$name.ppm"
    if [[ "$name" == editor ]]; then
        timeout 60 "$root/target/debug/examples/$name" --smoke > "$out/scenarios/$name.log" 2>&1
    else
        timeout 60 "$root/target/debug/examples/$name" > "$out/scenarios/$name.log" 2>&1
    fi
    result=$?
    printf '%s\t%s\n' "$name" "$result" >> "$out/scenario-results.tsv"
    echo "$name: $result"
done
