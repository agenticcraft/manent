---
id: VRT-4
title: 'Audio capture core: AudioSource trait + Linux sources'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - audio
  - backend
  - linux
milestone: m-1
dependencies:
  - VRT-1
  - VRT-3
documentation:
  - doc-2
priority: high
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the audio module: AudioSource trait, device enumeration (SourceInfo {id, name, kind: Mic|Loopback}), Linux implementations for mic and PipeWire monitor, and lock-free ring buffers (rtrb) so realtime callbacks never block. See doc-2 Technical architecture §1.2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tauri command lists available mic and loopback sources
- [ ] #2 Starting/stopping a source delivers samples through a ring buffer without blocking the audio callback
- [ ] #3 Multiple sources can run concurrently
- [ ] #4 Unit tests cover ring buffer and enumeration mapping
<!-- AC:END -->
