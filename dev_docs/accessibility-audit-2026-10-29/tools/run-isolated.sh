#!/usr/bin/env bash
# Private virtual KWin, private D-Bus, and disposable application configuration.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
out=${LHC_AUDIT_OUT:?set absolute evidence directory}
run_root=$(mktemp -d /tmp/lhc-a11y-session.XXXXXX)
cleanup() {
    local attempt
    # D-Bus-activated services can create a last runtime file while they exit.
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if rm -rf -- "$run_root" 2>/dev/null; then return 0; fi
        sleep 0.2
    done
    printf 'Temporary session cleanup failed: %s\n' "$run_root" >&2
    return 1
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$run_root/runtime"
export XDG_CONFIG_HOME="$run_root/config"
export XDG_CACHE_HOME="$run_root/cache"
export XDG_DATA_HOME="$run_root/data"
export LHC_DEV_DIR="$run_root/lhc"
export LHC_SOCKET="$run_root/lhc.sock"
export LHC_HOTKEYS=off
export SLINT_BACKEND=${SLINT_BACKEND:-winit-software}
unset NO_AT_BRIDGE
export QT_ACCESSIBILITY=1
export ATSPI_DBUS_IMPLEMENTATION=dbus-daemon
export LHC_AUDIT_ROOT="$repo"
mkdir -m 700 -p "$XDG_RUNTIME_DIR"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$LHC_DEV_DIR"
export LHC_AUDIT_COMMAND=${LHC_AUDIT_COMMAND:-"$repo/target/debug/examples/a11y_probe"}
cat > "$run_root/session" <<'SESSION'
#!/usr/bin/env bash
set -euo pipefail
exec bash -c "$LHC_AUDIT_COMMAND"
SESSION
chmod +x "$run_root/session"
dbus-run-session -- kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --no-kactivities --socket "lhc-a11y-$$" --width "${LHC_AUDIT_WIDTH:-1920}" --height "${LHC_AUDIT_HEIGHT:-1080}" --output-count 1 --exit-with-session "$run_root/session" > "$out/kwin-session${LHC_AUDIT_SUFFIX:-}.log" 2>&1
