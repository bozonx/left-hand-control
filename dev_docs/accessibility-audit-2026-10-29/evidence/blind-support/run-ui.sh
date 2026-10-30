#!/usr/bin/env bash
set -u
out=/tmp/lhc-blind-validation
mkdir -p "$out/probe"
: > "$out/ui-results.tsv"
for name in accessibility keyboard-navigation; do
    timeout 60 "target/debug/examples/$name" > "$out/$name.log" 2>&1
    printf '%s\t%s\n' "$name" "$?" >> "$out/ui-results.tsv"
done
timeout 45 python3 dev_docs/accessibility-audit-2026-10-29/tools/check-accessibility.py > "$out/atspi-fixture.log" 2>&1
printf 'atspi-fixture\t%s\n' "$?" >> "$out/ui-results.tsv"
LHC_AUDIT_OUT="$out/probe" timeout 90 python3 "$out/atspi-watch.py" > "$out/probe.log" 2>&1
printf 'atspi-probe\t%s\n' "$?" >> "$out/ui-results.tsv"
bash dev_docs/accessibility-audit-2026-10-29/tools/run-scenarios.sh
