#!/usr/bin/env bash
set -u
out=/tmp/lhc-blind-validation
: > "$out/cargo-results.tsv"
run_check() {
    local name=$1
    shift
    "$@" > "$out/$name.log" 2>&1
    printf '%s\t%s\n' "$name" "$?" >> "$out/cargo-results.tsv"
}
run_check fmt cargo fmt -p lhc-core -p slint-shell --check
run_check clippy cargo clippy --locked -p lhc-core -p slint-shell --features slint-shell/spell --all-targets --no-deps -- -D warnings
run_check workspace-tests cargo test --locked --workspace --features slint-shell/spell
run_check non-spell cargo check --locked -p slint-shell --all-targets
run_check deny cargo deny check
