---
id: VRT-26
title: Mobile companion app (Android/iOS)
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T11:55:00Z'
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
Mobile is a companion, not a second Veritas (decision-4, 2026-11-01). The phone records a meeting; the user's desktop Veritas transcribes it (speakers, recognised people, search); the transcript comes back to the phone. No engine or model on the phone.
Two ways for phone and desktop to exchange recordings and transcripts, both built into Veritas itself:
- **Nextcloud folder** (decision-5): both apps sync with a folder on the user's own Nextcloud over WebDAV, without the Nextcloud client apps. The phone puts recordings in Veritas/Inbox/, the desktop transcribes them and writes transcript.md (Markdown with a YAML header, VRT-37) next to the audio, which the phone reads and searches. Works when the desktop is not on the same network. VRT-38 (spike), VRT-39 (desktop).
- **Direct pairing on the local network** (decision-4): VRT-26.1 (spike and decision), VRT-26.2 (desktop companion service).
Phone side: VRT-26.3 (record and send), VRT-26.4 (transcripts on the phone). Audio and transcripts go only between the user's own devices and, when the user enables it, the user's own Nextcloud; never voice embeddings, never a third-party cloud.
Android: mic, plus system audio through AudioPlaybackCapture where apps allow it. iOS: mic only (OS limits). Live captions on the phone are not in the first version.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A meeting recorded on the phone ends up as a transcribed meeting on the desktop, with known people recognised
- [ ] #2 The phone shows and searches the finished transcripts
- [ ] #3 Data goes only between the user's own devices and, if enabled, the user's own Nextcloud; pairing and the Nextcloud connection can be revoked
<!-- AC:END -->
