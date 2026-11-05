---
id: VRT-23
title: 'Packaging and updates: Fedora RPM from a signed dnf repository'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-11-05T22:01:00Z'
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
- [x] #1 An RPM with the app, the engine helper and the licence, signed, in a dnf repository with signed metadata (scripts/release-rpm.sh, decision-9)
- [x] #2 Installing from the repository and upgrading from one release to the next works with dnf (0.1.0 -> 0.1.1 -> 0.1.2 -> 0.1.3), keeping the user's data
- [x] #3 Packaged build checked in the app switcher and launcher: the Manent icon from the .desktop file (VRT-43)
- [x] #4 macOS signing/notarization, the Windows installer with libparakeet from source, and CI installers are split off (VRT-50, VRT-51, VRT-13)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope narrowed to Fedora (decision-9, decision 2026-11-04). Verified by hand: dnf install from the local test repository (gpgcheck + repo_gpgcheck), upgrades to 0.1.1, 0.1.2 and 0.1.3, data kept, icon in the launcher and switcher.

While the repository is only on the local test server, dnf can't always reach it: on 2026-11-05 it was found disabled (manent.enabled=0 in /etc/dnf/repos.override.d/99-config_manager.repo) and upgrades silently did nothing. Test upgrades use --enablerepo=manent. Once it is hosted on agenticcraft.ai: sudo dnf config-manager setopt manent.enabled=1, and add the repo again with the real URL.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fedora first: Tauri builds an RPM that also ships manent-engine and the GPL-3.0-or-later licence; scripts/release-rpm.sh makes the signing key once (outside the repo), signs the RPM, builds and signs the repo metadata and writes manent.repo and the public key, so users add the repo once and update with dnf or GNOME Software (no in-app updater, decision-9). Tested by installing and upgrading 0.1.0 -> 0.1.3 from a local test repository on the test machine, data and icon OK. macOS, Windows and CI installers moved to VRT-50, VRT-51 and VRT-13. AGENTS.md has a Releases section.
<!-- SECTION:FINAL_SUMMARY:END -->
