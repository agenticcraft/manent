#!/bin/sh
# Signs the engine helper before Tauri bundles it into Manent.app (VRT-12). Tauri signs the app
# itself but not the extra files it copies, and notarization needs every executable signed with
# the hardened runtime. The helper gets its own entitlements (library validation off, so it can
# load the downloaded libparakeet); the app keeps library validation.
#
# Run by `npm run tauri build` on macOS (build.beforeBundleCommand in tauri.macos.conf.json),
# from the repository root. Identity: $APPLE_SIGNING_IDENTITY (a Developer ID for releases),
# else ad-hoc ("-") for local tests.
set -eu

helper=src-tauri/target/release/manent-engine
identity=${APPLE_SIGNING_IDENTITY:--}
# Notarization needs a secure timestamp; an ad-hoc signature can't have one.
if [ "$identity" = "-" ]; then timestamp=--timestamp=none; else timestamp=--timestamp; fi

codesign --force --options runtime "$timestamp" \
  --entitlements src-tauri/macos/manent-engine.entitlements \
  --sign "$identity" "$helper"
