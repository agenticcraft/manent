---
id: VRT-39
title: 'Desktop: Nextcloud sync of meetings and the phone inbox'
status: To Do
assignee: []
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-01T11:55:00Z'
labels:
  - sync
milestone: m-4
dependencies:
  - VRT-38
  - VRT-37
priority: medium
ordinal: 43000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-5, following the spike VRT-38. Settings gets a Sync section, off by default: 'Connect Nextcloud' (Login Flow v2), the folder (default Veritas/), and what to sync: recordings from the phone only, or also all desktop meetings (audio playback file, meta.json, transcript.md; never the database or voice embeddings). The desktop watches Veritas/Inbox/: each new phone recording becomes a meeting through the import job (VRT-19), so known people are recognised; afterwards the recording moves to its meeting folder with transcript.md (VRT-37). Changed transcripts (naming, re-transcribe) are uploaded again. Status shown in Settings and the Library (last sync, errors); works offline and catches up later. Disconnect removes the app password from the keychain.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Off by default; connect via Login Flow v2; app password in the keychain; disconnect revokes it
- [ ] #2 A recording put in the inbox becomes a transcribed meeting; its transcript.md lands next to it in Nextcloud
- [ ] #3 Optionally all desktop meetings are synced (playback file + transcript.md), updates follow naming and re-transcribe
- [ ] #4 Offline and errors handled; smoke test against a local Nextcloud container
<!-- AC:END -->
