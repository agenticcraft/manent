#!/bin/bash
# Inside dbus-run-session: start a headless GNOME Shell, run "$@" against it, stop it.
D=$(dirname "$0"); LOG=$D/out; mkdir -p "$LOG"
MUTTER_DEBUG=${MUTTER_DEBUG_TOPICS:-} gnome-shell --headless --wayland --virtual-monitor 1280x800 --wayland-display wl-test > "$LOG/shell.log" 2>&1 &
SHELL_PID=$!
for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/wl-test" ] && break; sleep 0.2; done
export WAYLAND_DISPLAY=wl-test
sleep 3
DISPLAY=$(grep -oE 'Using public X11 display :[0-9]+' "$LOG/shell.log" | grep -oE ':[0-9]+' | head -1); export DISPLAY
"$@"; rc=$?
kill $SHELL_PID; wait $SHELL_PID 2>/dev/null
exit $rc
