---
id: VRT-33
title: Own download mirror for the engine and model
status: To Do
assignee: []
created_date: '2026-10-30T08:30:00Z'
labels:
  - backlog
  - release
milestone: m-4
dependencies:
  - VRT-29
priority: low
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Host the engine library and the model (parakeet-cpp-bundle-standard, ~1.1 GB) on our own mirror (e.g. a bucket) for speed and control, with Hugging Face as fallback. Not now: the app downloads from the official sources first (decision 2026-10-30, VRT-29). Keep checksums identical to the official files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Mirror hosts the same files with identical sha256
- [ ] #2 App tries the mirror first and falls back to the official source
<!-- AC:END -->
