#!/bin/bash
# Run a probe in a headless GNOME session with its own D-Bus, runtime dir, home and portals,
# so nothing touches the user's session, clipboard or settings. Example:
#   spikes/dictation/run-isolated.sh python3 spikes/dictation/autopaste.py
D=$(dirname "$(readlink -f "$0")"); W=$D/out/sandbox
RT=/tmp/manent-dict-$UID   # Wayland socket paths are limited to 108 bytes
mkdir -p "$W"/{home,cfg,data/applications,cache} "$RT"; chmod 700 "$RT"
printf '[Desktop Entry]\nType=Application\nName=Dictation probe\nExec=/bin/true\nNoDisplay=true\n' > "$W/data/applications/test.manent.probe.desktop"
exec env -i PATH=/usr/bin:/bin HOME="$W/home" XDG_RUNTIME_DIR="$RT" XDG_CONFIG_HOME="$W/cfg" XDG_DATA_HOME="$W/data" \
  XDG_CACHE_HOME="$W/cache" XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=GNOME LANG=en_US.UTF-8 GTK_A11Y=none \
  MUTTER_DEBUG_TOPICS="${MUTTER_DEBUG_TOPICS:-}" ALSO_PRIMARY="${ALSO_PRIMARY:-}" \
  PIPEWIRE_RUNTIME_DIR="${PIPEWIRE_RUNTIME_DIR:-}" CARGO_HOME="${CARGO_HOME:-}" RUSTUP_HOME="${RUSTUP_HOME:-}" \
  dbus-run-session -- "$D/session.sh" "$@"
