---
id: VRT-14
title: LocalAI sidecar manager and model download
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - transcription
  - backend
milestone: m-2
dependencies:
  - VRT-2
  - VRT-3
documentation:
  - doc-2
priority: high
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Bundle the local-ai binary via Tauri externalBin, start it on 127.0.0.1 with a random port, health-check and restart it, and install parakeet-cpp-bundle-standard on first run with progress shown in the UI. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sidecar starts with the app and stops on exit
- [ ] #2 Crash is detected and the sidecar restarted
- [ ] #3 First-run model download shows progress and survives app restart
- [ ] #4 Binds only to localhost
<!-- AC:END -->
