---
id: VRT-56
title: Publish the RPMs on GitHub Pages
status: In Progress
assignee: []
created_date: '2026-11-06T20:22:00Z'
updated_date: '2026-11-06T20:49:00Z'
labels:
  - packaging
milestone: m-3
dependencies: []
priority: high
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Requested (2026-11-06): the dnf repository goes on GitHub Pages of the public repository github.com/agenticcraft/manent instead of agenticcraft.ai (decision-9), so users can install and update with dnf. The repository is static files, signed locally; the private key never leaves the computer. Test builds (0.1.0-0.1.5, local URL) stay out of it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 scripts/release-rpm.sh --publish pushes the signed repository to the gh-pages branch, with no private key in it; test releases (--base-url) stay in a separate local folder
- [x] #2 The first public release is 0.1.6, higher than every test build
- [x] #3 manent.repo, the key, the metadata and the RPM are served from https://agenticcraft.github.io/manent/rpm and their signatures verify
- [ ] #4 A real install or upgrade from that URL works with dnf
- [x] #5 decision-9, AGENTS.md and the README describe it
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Move the test repository to repo-test; default base URL becomes the Pages URL; --publish clones or creates gh-pages, syncs repo/ to rpm/, adds .nojekyll and a small index, commits and pushes. Bump to 0.1.6.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Published 0.1.6 to gh-pages (2026-11-06). All files return 200 at https://agenticcraft.github.io/manent/rpm; repomd.xml.asc and the RPM verify against the published key; the branch holds only the RPM, metadata, public key, manent.repo, index and .nojekyll. Open: a real dnf install from that URL.

gh-pages replaced by one commit with 0.1.7 (2026-11-06): build paths removed from the binaries (RUSTFLAGS and CFLAGS in release-rpm.sh). Test fixtures added to git. Open: a real dnf install from the public URL.
<!-- SECTION:NOTES:END -->
