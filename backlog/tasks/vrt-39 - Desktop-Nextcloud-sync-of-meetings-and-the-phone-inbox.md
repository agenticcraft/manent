---
id: VRT-39
title: 'Desktop: Nextcloud sync of meetings and the phone inbox'
status: To Do
assignee: []
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-02T20:02:00Z'
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
decision-5, built as decided in decision-7 (spike VRT-38; the prototype in spikes/nextcloud is the starting point: dav.rs, Login Flow v2, keyring). Settings gets a Sync section, off by default:
- 'Connect Nextcloud' (Login Flow v2: the browser opens; the app password goes only into the OS keychain), the folder (default Veritas/), and what to sync: recordings from the phone only, or also all desktop meetings (mix.opus, transcript.md; never the database or voice embeddings).
- The desktop polls Veritas/ by ETag (one Depth-0 PROPFIND per round at the server's pollinterval, 60 s by default; at once after its own writes and when the app comes to the front). Each new recording in Veritas/Inbox/ (with its meta.json) becomes a meeting through the import job (VRT-19), so known people are recognised; then the recording is MOVEd on the server into Veritas/<YYYY-MM-DD HH.MM> <title>/ and transcript.md (VRT-37) is written next to it.
- Changed transcripts (naming, re-transcribe, title) are uploaded again; large files with chunked upload v2 (resumable), downloads resume with Range/If-Match.
- Deleting a meeting asks whether to remove it from Nextcloud too; a folder deleted in Nextcloud is not deleted on the desktop or uploaded again unasked.
- Status in Settings and the Library (last sync, errors); works offline and catches up later. Disconnect revokes the app password on the server and removes it from the keychain.
- To settle while building: servers with a self-signed certificate (pinning or a CA option), and a two-factor account through Login Flow v2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Off by default; connect via Login Flow v2; app password in the keychain; disconnect revokes it
- [ ] #2 A recording put in the inbox becomes a transcribed meeting; its transcript.md lands next to it in Nextcloud
- [ ] #3 Optionally all desktop meetings are synced (playback file + transcript.md), updates follow naming and re-transcribe
- [ ] #4 Offline and errors handled; smoke test against a local Nextcloud container
<!-- AC:END -->
