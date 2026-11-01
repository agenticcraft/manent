---
id: VRT-26
title: Mobile companion app (Android/iOS)
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T10:08:00Z'
labels:
  - backlog
  - mobile
milestone: m-4
dependencies: []
documentation:
  - doc-1
priority: low
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Mobile is a companion, not a second Veritas (decision-4, 2026-11-01). The phone records a meeting and sends it to the user's paired desktop Veritas, which transcribes it: speakers, recognised people, search. The finished transcript then comes back to the phone. No engine or model on the phone; audio and transcripts travel only between the user's own paired devices, never through a cloud service.
Android: mic, plus system audio through AudioPlaybackCapture where apps allow it. iOS: mic only (OS limits).
Live captions on the phone are not in the first version; streaming while on the same network could come later.
Subtasks: VRT-26.1 pairing and transport (spike and decision), VRT-26.2 desktop companion service, VRT-26.3 mobile app (record and send), VRT-26.4 transcripts on the phone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A meeting recorded on the phone ends up as a transcribed meeting on the paired desktop, with known people recognised
- [ ] #2 The phone shows the finished transcript
- [ ] #3 Nothing passes through a cloud service; pairing can be revoked from both sides
<!-- AC:END -->
