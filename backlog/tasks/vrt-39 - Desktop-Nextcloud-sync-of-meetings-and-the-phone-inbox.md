---
id: VRT-39
title: 'Desktop: Nextcloud sync of meetings and the phone inbox'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-03T19:37:00Z'
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
- [x] #1 Off by default; connect via Login Flow v2; app password in the keychain; disconnect revokes it
- [x] #2 A recording put in the inbox becomes a transcribed meeting; its transcript.md lands next to it in Nextcloud
- [x] #3 Optionally all desktop meetings are synced (playback file + transcript.md), updates follow naming and re-transcribe
- [x] #4 Offline and errors handled; smoke test against a local Nextcloud container
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built on the spike (decision-7; settled points added to decision-7).

**Rust**
- **sync/dav.rs:** the WebDAV client from spikes/nextcloud. It checks certificates against the OS trust store (ureq platform-verifier) and adds Login Flow v2 calls.
- **sync/mod.rs:** the service. A round runs every 60 s, and right away after a transcript write (transcript_md calls sync::wake), on Sync now, on window focus and on connect:
  - make the folders;
  - compare the Depth-0 ETag;
  - fetch the inbox (meta.json gives title and start time; Jobs::import_as);
  - finish imports: MOVE the recording and its meta.json into '<YYYY-MM-DD HH.MM> <title>/';
  - in 'all' mode, add desktop meetings;
  - upload changed transcript.md by hash, and mix.opus once (chunked v2).
- **Edge cases:**
  - 404/409 on upload marks a folder gone; it is left alone.
  - A failed import is retried when the file's ETag changes.
  - Rounds wait for the UI's UTC offset (folder names in local time).
  - Errors read as sentences (friendly()), with details in the log.
- **Data and commands:**
  - migration 0005: sync_meetings, sync_inbox;
  - new commands: sync_status, sync_connect, sync_open_login, sync_cancel_connect, sync_disconnect, sync_set_mode, sync_now, set_utc_offset;
  - delete_meeting(alsoRemote); MeetingSummary.synced.
  - open_url.rs is the OS browser opener; VERITAS_NO_BROWSER skips it for tests.
- **Disconnect** revokes the app password and clears the keychain entry, the settings (also the mode, so a new connection starts phone-only) and the sync tables.

**UI**
- Settings > Nextcloud: connect, the waiting state, the account, last sync, the http warning, errors, What to sync, Disconnect with confirmation.
- Library: SyncLine.
- The delete confirmation has an 'Also from Nextcloud' checkbox.

**Found and fixed on the way**
- Opus files couldn't be imported at all (symphonia has no Opus decoder). They are now decoded with libopus, in Ogg and WebM, with new fixtures tone.opus and tone-stereo.webm in the import test.
- A failed import left a reference to a deleted meeting (foreign key error).

**Tests**
- scripts/smoke-sync.py PASS against the local Nextcloud 35 container:
  - connect via Login Flow v2; the keychain holds the app password and the database doesn't;
  - an inbox recording with meta.json is imported with its title and time, transcribed, and MOVEd into '2026-11-03 19.36 Smoke phone meeting/' with transcript.md;
  - offline (container stopped): the Library shows the error; online again, sync recovers;
  - naming a speaker updates transcript.md;
  - all meetings: 12 desktop meetings with mix.opus + transcript.md;
  - a folder deleted on the server is not uploaded again;
  - delete with alsoRemote removes the folder;
  - disconnect: the server answers 401 to the old password and the keychain entry is gone.
- smoke-import.py PASS with Opus, WebM and MP3 speech.
- 125 Rust tests, clippy 0, npm check green.

**Not covered**
- a real 2FA account;
- a server with a self-signed certificate in the OS store;
- the phone side (VRT-26.3/26.4).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Veritas syncs with the user's own Nextcloud without the Nextcloud apps: connect in Settings, phone recordings in the inbox become transcribed meetings with transcript.md next to them on the server, and optionally every meeting syncs. It works offline and catches up, and disconnecting revokes access. Opus imports work now.
<!-- SECTION:FINAL_SUMMARY:END -->
