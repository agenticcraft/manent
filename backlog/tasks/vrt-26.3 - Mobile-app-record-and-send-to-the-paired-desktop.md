---
id: VRT-26.3
title: 'Mobile app: record and send to the paired desktop'
status: To Do
assignee: []
created_date: '2026-11-01T10:07:00Z'
labels:
  - mobile
  - frontend
milestone: m-4
dependencies:
  - VRT-26.1
  - VRT-26.2
parent_task_id: VRT-26
priority: low
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The phone side of the companion (decision-4). It pairs by scanning the desktop's QR code, records a meeting (mic; on Android also system audio via AudioPlaybackCapture where allowed; Opus), and keeps recording with the screen off. It queues the recording and sends it when the paired desktop is reachable, with progress, and deletes its local copy once the desktop has it safely (privacy). Calm UI following doc-3. Platform choice (Tauri mobile or native) per the VRT-26.1 decision.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pair by QR code; record with the screen off; title and date kept
- [ ] #2 Recordings are queued and sent when the desktop is reachable, with progress, and removed from the phone after a confirmed transfer
- [ ] #3 Works on Android; iOS records the mic
<!-- AC:END -->
