---
id: VRT-14
title: LocalAI sidecar manager and model download
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T17:05:00Z'
labels:
  - transcription
  - backend
milestone: m-2
dependencies:
  - VRT-3
  - VRT-29
  - VRT-32
documentation:
  - doc-2
priority: high
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run the transcription engine as a sidecar for Dutch and English: start it on 127.0.0.1 with a random port, health-check and restart it, and install the CPU backend (cpu-parakeet-cpp, never the auto-detected GPU builds) and the models for both languages on demand with progress shown in the UI. Model set from VRT-32 (validated so far: English live realtime_eou_120m, offline parakeet-cpp-bundle-standard for NL+EN; Dutch live candidate nemotron-3.5 streaming). Packaging and delivery (bundled vs downloaded after install, full LocalAI vs leaner) follow the VRT-29 decision. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sidecar starts with the app and stops on exit
- [ ] #2 Crash is detected and the sidecar restarted
- [ ] #3 Model download for Dutch and English shows progress, verifies checksums and survives app restart
- [ ] #4 Binds only to localhost
<!-- AC:END -->
