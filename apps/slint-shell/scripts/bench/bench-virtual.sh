#!/usr/bin/env bash
set -euo pipefail
export TECLOID_PROTOTYPE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export TECLOID_WORKSPACE_ROOT="$(cd "$TECLOID_PROTOTYPE_ROOT/../.." && pwd)"
export TECLOID_PROTOTYPE_RESULTS="$(realpath -m "${1:?output directory required}")"
export TECLOID_VIRTUAL_TASK="${2:-geometry}"
export TECLOID_SLINT_SHELL_BIN="${TECLOID_BIN:-$TECLOID_WORKSPACE_ROOT/target/debug/tecloid}"
export TECLOID_BENCH_RETURN_BIN="${TECLOID_BENCH_RETURN_BIN:-$TECLOID_WORKSPACE_ROOT/target/debug/examples/bench-return}"
case "$TECLOID_VIRTUAL_TASK" in geometry|lifecycle|input) ;; *) exit 2 ;; esac
tecloid_test_root="$(mktemp -d)"
cleanup() {
    for attempt in 1 2 3; do
        rm -rf -- "$tecloid_test_root" 2>/dev/null && return
        sleep 0.2
    done
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$tecloid_test_root/runtime"
export XDG_CONFIG_HOME="$tecloid_test_root/config"
export XDG_CACHE_HOME="$tecloid_test_root/cache"
export XDG_DATA_HOME="$tecloid_test_root/data"
mkdir -m 700 "$XDG_RUNTIME_DIR"
if [[ "$TECLOID_VIRTUAL_TASK" == input ]]; then
    export KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
fi
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
cat > "$XDG_CONFIG_HOME/kxkbrc" <<'LAYOUT'
[Layout]
Use=true
LayoutList=us,ru
Model=pc104
LAYOUT
cat > "$tecloid_test_root/session" <<'SESSION'
#!/usr/bin/env bash
set -euo pipefail
export TECLOID_HOTKEYS=off
export RUST_LOG=warn
if [[ "$TECLOID_VIRTUAL_TASK" == geometry ]]; then
    exec python3 "$TECLOID_PROTOTYPE_ROOT/scripts/bench/bench-geometry.py" "$TECLOID_PROTOTYPE_RESULTS"
elif [[ "$TECLOID_VIRTUAL_TASK" == input ]]; then
    mkdir -p "$TECLOID_PROTOTYPE_RESULTS"
    export TECLOID_ISOLATED_INPUT=1
    export SLINT_BACKEND=winit-skia-software
    exec "$TECLOID_BENCH_RETURN_BIN" "$TECLOID_SLINT_SHELL_BIN" "$TECLOID_PROTOTYPE_RESULTS/parent.csv"
else
    exec python3 "$TECLOID_PROTOTYPE_ROOT/scripts/bench/bench-lifecycle.py" "$TECLOID_PROTOTYPE_RESULTS" 500
fi
SESSION
chmod +x "$tecloid_test_root/session"
dbus-run-session -- kwin_wayland --virtual --no-lockscreen --no-global-shortcuts \
    --no-kactivities --socket "tecloid-stage3a-$$" --width 1920 --height 1080 \
    --output-count 2 --exit-with-session "$tecloid_test_root/session"
