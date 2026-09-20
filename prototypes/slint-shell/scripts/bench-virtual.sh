#!/usr/bin/env bash
set -euo pipefail
export LHC_PROTOTYPE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export LHC_PROTOTYPE_RESULTS="$(realpath -m "${1:?output directory required}")"
export LHC_VIRTUAL_TASK="${2:-geometry}"
case "$LHC_VIRTUAL_TASK" in geometry|lifecycle|input) ;; *) exit 2 ;; esac
lhc_test_root="$(mktemp -d)"
cleanup() {
    for attempt in 1 2 3; do
        rm -rf -- "$lhc_test_root" 2>/dev/null && return
        sleep 0.2
    done
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$lhc_test_root/runtime"
export XDG_CONFIG_HOME="$lhc_test_root/config"
export XDG_CACHE_HOME="$lhc_test_root/cache"
export XDG_DATA_HOME="$lhc_test_root/data"
mkdir -m 700 "$XDG_RUNTIME_DIR"
if [[ "$LHC_VIRTUAL_TASK" == input ]]; then
    export KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
fi
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
cat > "$XDG_CONFIG_HOME/kxkbrc" <<'LAYOUT'
[Layout]
Use=true
LayoutList=us,ru
Model=pc104
LAYOUT
cat > "$lhc_test_root/session" <<'SESSION'
#!/usr/bin/env bash
set -euo pipefail
export SLINT_SHELL_HOTKEYS=off
export RUST_LOG=warn
if [[ "$LHC_VIRTUAL_TASK" == geometry ]]; then
    exec python3 "$LHC_PROTOTYPE_ROOT/scripts/bench-geometry.py" "$LHC_PROTOTYPE_RESULTS"
elif [[ "$LHC_VIRTUAL_TASK" == input ]]; then
    mkdir -p "$LHC_PROTOTYPE_RESULTS"
    export SLINT_SHELL_ISOLATED_INPUT=1
    export SLINT_BACKEND=winit-software
    exec "$LHC_PROTOTYPE_ROOT/target/debug/examples/bench-return" "$LHC_PROTOTYPE_ROOT/target/debug/slint-shell" "$LHC_PROTOTYPE_RESULTS/parent.csv"
else
    exec python3 "$LHC_PROTOTYPE_ROOT/scripts/bench-lifecycle.py" "$LHC_PROTOTYPE_RESULTS" 500
fi
SESSION
chmod +x "$lhc_test_root/session"
dbus-run-session -- kwin_wayland --virtual --no-lockscreen --no-global-shortcuts \
    --no-kactivities --socket "lhc-stage3a-$$" --width 1920 --height 1080 \
    --output-count 2 --exit-with-session "$lhc_test_root/session"
