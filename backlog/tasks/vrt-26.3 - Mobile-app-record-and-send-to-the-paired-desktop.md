---
id: VRT-26.3
title: 'Mobile app: record and send to the desktop'
status: To Do
assignee: []
created_date: '2026-11-01T10:07:00Z'
updated_date: '2026-11-01T11:41:00Z'
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
The phone side of the companion (decision-4, decision-5). It records a meeting (mic; on Android also system audio via AudioPlaybackCapture where allowed; Opus) and keeps recording with the screen off. It sends the recording to the desktop by the route the user set up: uploaded to Veritas/Inbox/ on the user's Nextcloud over WebDAV with a meta.json (title, date, device), built into the app (VRT-38), or directly to a paired desktop on the local network (VRT-26.1). It queues recordings while offline and sends them later with progress and resume, and deletes its local copy once the transfer is confirmed (privacy). Calm UI following doc-3. Platform choice (Tauri mobile or native) per the VRT-26.1 / VRT-38 decisions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Records with the screen off; title and date kept
- [ ] #2 Connect to Nextcloud from the app (Login Flow v2, no Nextcloud client app needed) and upload to the inbox with resume
- [ ] #3 Recordings are queued while offline, sent with progress, and removed from the phone after a confirmed transfer
- [ ] #4 Works on Android; iOS records the mic
<!-- AC:END -->
