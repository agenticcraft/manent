---
id: VRT-38
title: 'Spike: Nextcloud sync over WebDAV, built into the app'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-02T20:02:00Z'
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
- [x] #1 Prototype: log in with Login Flow v2, upload a 1-hour Opus file with resume, detect a change by ETag, download a transcript.md
- [x] #2 Decision recorded: HTTP/WebDAV library, change detection, folder layout and ownership, encryption, keychain storage
- [x] #3 Works against a local Nextcloud container; no real account needed for tests
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Prototype in spikes/nextcloud (README there).
- **Code:** nc-spike CLI over ureq 3 + quick-xml + keyring 4: dav.rs (~300 lines); Login Flow v2; keychain.
- **Server:** local Nextcloud 35.0.2 in Docker (nextcloud:apache, SQLite, 127.0.0.1:8471) with a test user; no real account anywhere.
- **e2e.py PASS:**
  - Login Flow v2: nc_grant.py approves it over HTTP as a browser would; the login form needs the Origin header. The app password is in Secret Service and not in the state file.
  - A 1-hour Opus recording (14.9 MB) in 5 MB chunks: interrupted after 1 of 3 chunks; the resume skipped it and the file on the server is identical (sha256).
  - Download interrupted after 3 MB: resumed at 3145728 with Range and If-Match.
  - Watch: a quiet round is 1 Depth-0 PROPFIND (folder ETags propagate upwards). After the desktop wrote transcript.md and MOVEd the recording from the inbox, 3 PROPFINDs found exactly: ADDED transcript.md, ADDED the moved recording, REMOVED the inbox file.
  - Revoke: OCS DELETE apppassword; the server then answers 401 to the old password, and the keychain entry is gone.
- **Lessons:**
  - ureq 3 needs allow_non_standard_methods(true) for PROPFIND/MOVE, and an explicit Content-Length for byte bodies.
  - quick-xml 0.42: local_name().as_ref() is &str; text through xml10_content().
  - keyring 4 'v1' brings about 100 crates (zbus); the app has 494 now and no zbus yet. VRT-40 needs zbus as well.
- **Decision:** decision-7 (library, login, keychain, uploads and downloads, polling, folder layout and ownership, deleting, encryption optional and off by default).
- **Updated:** AGENTS.md local-first rule and VRT-39.
- **Not covered:** the phone side (VRT-26.3), self-signed certificates and 2FA accounts (listed in VRT-39).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Nextcloud sync is proven against a local server: Login Flow v2 with the app password in the keychain, resumable chunked uploads and Range downloads, change detection for one PROPFIND per quiet round, server-side moves, and revoking. Decided in decision-7; VRT-39 can build on spikes/nextcloud.
<!-- SECTION:FINAL_SUMMARY:END -->
