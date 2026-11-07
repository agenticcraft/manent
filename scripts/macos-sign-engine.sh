#!/bin/sh
# Signs the engine helper before Tauri bundles it into Manent.app (VRT-12). Tauri signs the app
# itself but not the extra files it copies, and notarization needs every executable signed with
# the hardened runtime. The helper gets its own entitlements (library validation off, so it can
# load the downloaded libparakeet); the app keeps library validation.
#
# Run by `npm run tauri build` on macOS (build.beforeBundleCommand in tauri.macos.conf.json),
# from the repository root. Tauri sets TAURI_ENV_DEBUG for --debug and TAURI_ENV_TARGET_TRIPLE;
# the helper of this build (target/<profile> or, with --target, target/<triple>/<profile>) is
# signed into target/bundle-engine/, where bundle.macOS.files takes it from.
# Identity: $APPLE_SIGNING_IDENTITY (a Developer ID for releases), else ad-hoc ("-").
set -eu

profile=release
if [ "${TAURI_ENV_DEBUG:-}" = "true" ]; then profile=debug; fi

# Cargo just built the helper of this build, so of the two places it can be, the newest wins.
helper=""
for candidate in "src-tauri/target/${TAURI_ENV_TARGET_TRIPLE:-none}/$profile/manent-engine" \
  "src-tauri/target/$profile/manent-engine"; do
  if [ -f "$candidate" ] && { [ -z "$helper" ] || [ "$candidate" -nt "$helper" ]; }; then
    helper=$candidate
  fi
done
if [ -z "$helper" ]; then
  echo "macos-sign-engine: no $profile build of manent-engine found" >&2
  exit 1
fi

out=src-tauri/target/bundle-engine
mkdir -p "$out"
cp "$helper" "$out/manent-engine"

identity=${APPLE_SIGNING_IDENTITY:--}
# Notarization needs a secure timestamp; an ad-hoc signature can't have one.
if [ "$identity" = "-" ]; then timestamp=--timestamp=none; else timestamp=--timestamp; fi

codesign --force --options runtime "$timestamp" \
  --entitlements src-tauri/macos/manent-engine.entitlements \
  --sign "$identity" "$out/manent-engine"
echo "macos-sign-engine: signed $helper"
