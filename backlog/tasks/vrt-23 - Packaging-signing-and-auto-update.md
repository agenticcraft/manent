---
id: VRT-23
title: Packaging, signing and auto-update
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-03T20:13:00Z'
labels:
  - release
milestone: m-3
dependencies:
  - VRT-13
  - VRT-14
documentation:
  - doc-1
priority: low
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Produce installers for Linux (AppImage/rpm/deb), Windows (MSI/NSIS) and macOS (dmg), with code signing/notarization and Tauri updater. The installer contains the app and the veritas-engine helper only; libparakeet and the model are downloaded at first run (decision-1). The helper must end up next to the app binary (engine::helper::helper_exe looks there), e.g. as a Tauri externalBin with the target-triple name. LocalAI publishes no Windows (nor macOS Intel, Linux arm64 CPU) build of libparakeet, so CI builds parakeet.cpp from source for Windows and hosts the result (with VRT-33). The engine pins in engine/assets.rs are part of a release: a new pin means re-running the engine integration tests.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Installers build in CI for all three platforms
- [ ] #2 macOS build signed and notarized
- [ ] #3 Auto-update works from one release to the next
- [ ] #4 Windows libparakeet built from source (parakeet.cpp, CMake) and served as a verified download
- [ ] #5 Packaged build checked in the app switcher and launcher: the Manent icon from the .desktop file (VRT-43)
<!-- AC:END -->
