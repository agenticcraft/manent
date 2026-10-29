---
id: VRT-14
title: LocalAI sidecar manager and model download
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T18:45:00Z'
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
Run the transcription engine as a sidecar on 127.0.0.1 with a random port and a raised --upload-limit (LocalAI default 15 MB), health-check and restart it, and install the CPU backend (cpu-parakeet-cpp, never the auto-detected GPU builds) and the one model for Dutch and English, parakeet-cpp-bundle-standard (1.1 GB, decided in VRT-32), on demand with progress, checksum verification and resume. Show the model attributions (NOTICE). Packaging and delivery (bundled vs downloaded after install, full LocalAI vs leaner) follow the VRT-29 decision. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sidecar starts with the app and stops on exit
- [ ] #2 Crash is detected and the sidecar restarted
- [ ] #3 Model download shows progress, verifies the checksum and survives app restart
- [ ] #4 Binds only to localhost and accepts uploads of a 2-hour meeting (FLAC)
- [ ] #5 Model attributions/licences shown in the app
<!-- AC:END -->
