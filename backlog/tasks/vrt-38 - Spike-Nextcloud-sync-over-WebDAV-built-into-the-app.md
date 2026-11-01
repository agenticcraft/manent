---
id: VRT-38
title: 'Spike: Nextcloud sync over WebDAV, built into the app'
status: To Do
assignee: []
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-01T11:55:00Z'
labels:
  - sync
  - spike
milestone: m-4
dependencies: []
priority: medium
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-5: Veritas syncs with a folder on the user's own Nextcloud itself, over WebDAV, on desktop and in the companion app; no dependency on the Nextcloud desktop or mobile clients. Find out and prototype:
- Login: Nextcloud Login Flow v2 (browser opens, the app gets an app password); store it in the OS keychain (desktop: keyring crate; Android Keystore / iOS Keychain); revoke = delete the app password.
- WebDAV from Rust: PROPFIND with ETags to see what changed (polling interval, or the notify_push app if installed), GET with Range, PUT; chunked upload v2 for large files with resume. Try ureq (decision-2) with raw WebDAV against a dedicated crate (reqwest_dav etc.); stack change needs a decision.
- Folder layout: Veritas/<meeting>/ with the audio, meta.json and transcript.md (VRT-37); Veritas/Inbox/ for phone recordings not yet transcribed. Who writes what (phone: audio + meta, desktop: transcript), so the two never write the same file; what deleting means on each side.
- Privacy: the server is the user's own; optional end-to-end encryption of the files by Veritas itself (key shared between the user's devices by QR code), since Nextcloud's E2EE is not usable over plain WebDAV. Decide whether it is required or optional.
- Test against a local Nextcloud in a container (no real account in tests).
Record the findings and a decision; update AGENTS.md's local-first rule.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Prototype: log in with Login Flow v2, upload a 1-hour Opus file with resume, detect a change by ETag, download a transcript.md
- [ ] #2 Decision recorded: HTTP/WebDAV library, change detection, folder layout and ownership, encryption, keychain storage
- [ ] #3 Works against a local Nextcloud container; no real account needed for tests
<!-- AC:END -->
