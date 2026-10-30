#!/usr/bin/env bash
# Runs UI scenarios one by one inside the private compositor.
set -u
out=$LHC_AUDIT_OUT
ex=$LHC_AUDIT_ROOT/target/debug/examples
for spec in "editor --smoke" interactions layer-keys library small-window; do
  set -- $spec; name=$1; shift
  timeout 180 "$ex/$name" "$@" > "$out/$name.log" 2>&1
  echo "$name exit=$?" >> "$out/summary.txt"
done
LHC_SMALL_RELEASE=1 timeout 180 "$ex/small-window" > "$out/small-window-release.log" 2>&1
echo "small-window (release UI) exit=$?" >> "$out/summary.txt"
