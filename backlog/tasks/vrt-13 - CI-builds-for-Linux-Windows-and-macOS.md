---
id: VRT-13
title: CI builds for Linux, Windows and macOS (Bitbucket Pipelines)
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T08:05:00Z'
labels:
  - ci
milestone: m-3
dependencies:
  - VRT-3
documentation:
  - doc-1
priority: low
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Bitbucket Pipelines (the repo lives on Bitbucket) that lint, test and build the Tauri app on Linux, Windows and macOS. Planned after the app works locally end to end (decision 2026-10-30). Windows/macOS steps need suitable runners (Bitbucket self-hosted or cloud runners); the libopus build needs cmake on every runner.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 bitbucket-pipelines.yml runs npm run check, cargo fmt/clippy and cargo test on push/PR
- [ ] #2 Build artifacts produced for Linux, Windows and macOS
<!-- AC:END -->
