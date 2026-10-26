#!/bin/sh
# Run the local (L3) desktop scenarios in the current graphical session with
# temporary configs, print a summary and write it to
# docs/testing/results/linux/<date>.txt. Usage: e2e-local.sh [--no-mapper]
set -u

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
cd -- "$repo_dir" || exit 1
if [ -z "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
    echo 'A graphical session is required.' >&2
    exit 1
fi
run_mapper=true
[ "${1:-}" = --no-mapper ] && run_mapper=false

logs=$(mktemp -d "${TMPDIR:-/tmp}/lhc-e2e-local.XXXXXXXX")
summary="$logs/summary.txt"
passed=0
failed=0

# run <name> <command…>: run one scenario, keep its log, record the outcome.
run() {
    name=$1
    shift
    printf '%-28s ' "$name"
    if "$@" > "$logs/$name.log" 2>&1; then
        result=passed
        passed=$((passed + 1))
    else
        result="FAILED (log: $logs/$name.log)"
        failed=$((failed + 1))
    fi
    echo "$result"
    echo "- $name: $result" >> "$summary"
}

echo 'Building the shell with Spell…'
if ! cargo build --locked -p slint-shell --features spell --examples --bins; then
    echo 'Build failed.' >&2
    exit 1
fi
binary=target/debug/left-hand-control
version=$("$binary" --version)
commit=$(git rev-parse --short HEAD)
git diff --quiet HEAD || commit="$commit (with uncommitted changes)"
plasma=$(plasmashell --version 2>/dev/null || echo 'not detected')
{
    echo "Date: $(date '+%F %T %z')"
    echo "Commit: $commit"
    echo "Version: $version"
    echo "Session: ${XDG_SESSION_TYPE:-unknown}, ${XDG_CURRENT_DESKTOP:-unknown}; $plasma"
    echo "Kernel: $(uname -sr)"
    echo
} > "$summary"

for example in 'editor --smoke' interactions library action-picker macros menus \
    undo-redo inline-edit drag-scroll notifications; do
    # shellcheck disable=SC2086 # split the example name from its arguments
    set -- $example
    name=$1
    shift
    run "example-$name" cargo run --locked -q -p slint-shell --features spell --example "$name" -- "$@"
done
run check-recovery python3 apps/slint-shell/scripts/check/check-recovery.py --binary "$binary" --popups spell
run check-preferences env LHC_BIN="$binary" python3 apps/slint-shell/scripts/check/check-preferences.py
run check-worker-crash python3 apps/slint-shell/scripts/check/check-worker-crash.py "$binary"
run check-linux-install python3 apps/slint-shell/scripts/release/check-linux-install.py
if $run_mapper; then
    run mapper-e2e cargo test --locked -q -p lhc-core --test mapper_e2e -- --ignored
fi

echo
echo "Passed: $passed, failed: $failed" | tee -a "$summary"
results=docs/testing/results/linux
mkdir -p -- "$results"
result_file="$results/$(date +%F).txt"
n=2
while [ -e "$result_file" ]; do
    result_file="$results/$(date +%F)-$n.txt"
    n=$((n + 1))
done
{
    cat -- "$summary"
    echo
    echo 'Manual smoke on the installed package (section 3 of'
    echo 'docs/testing/manual-acceptance.md): not recorded yet.'
} > "$result_file"
echo "Summary: $result_file"
[ "$failed" -eq 0 ]
