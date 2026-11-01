---
id: decision-5
title: >-
  Nextcloud folder sync built into desktop and companion; transcript as
  Markdown next to the audio
date: '2026-11-01T11:40:00Z'
status: accepted
---
## Context

decision-4 made mobile a companion that gets its recordings transcribed by the user's desktop, over a direct, paired connection on the local network. That only works while phone and desktop are on the same network, and needs discovery, pairing and certificates.

On 2026-11-01 the user proposed a second route: a folder on their own Nextcloud as the place where phone and desktop meet. Two conditions from the user:
- Sync is built into Veritas itself, on desktop and in the companion app. It must not depend on the Nextcloud desktop or mobile client apps.
- The transcription result is stored next to the original audio, so the phone can show and search it. First proposed as JSON; the user then chose Markdown with structured information in a YAML header (same day), so a transcript is readable by people and tools without Veritas.

## Decision

- **Nextcloud sync is part of Veritas.** Both apps talk WebDAV to the user's Nextcloud themselves. Login uses Nextcloud Login Flow v2: the user signs in in the browser and Veritas gets an app password, kept in the OS keychain. Off by default.
- **Folder layout:**
  - `Veritas/Inbox/` holds phone recordings waiting for transcription.
  - `Veritas/<meeting>/` holds the audio, `meta.json` and `transcript.md`.
  - Each file has one writer. The phone writes the audio and `meta.json`, and the desktop writes `transcript.md`, so the two apps never edit the same file.
- **`transcript.md` lives next to the audio in every meeting folder**, locally too (VRT-37). It is the Markdown export of VRT-21 with a versioned YAML header (id, title, start, duration, audio files, pass, speakers) and a block per speaker turn with its time. It is readable in Nextcloud's web viewer, findable with Nextcloud's search, and parseable by the phone. It holds speaker names and labels, **never voice embeddings**: biometric data stays in the desktop database.
- **No word-level timings in it:** the phone seeks per block. If word highlighting is wanted later, an optional `transcript.words.json` sidecar is added.
- **What syncs is the user's choice:** only the phone's recordings, or also all desktop meetings (the playback file and the JSON). The database never syncs.
- **Settled in the spike (VRT-38),** each with its own decision before building:
  - the WebDAV/HTTP library;
  - change detection (ETags, polling, notify_push);
  - chunked uploads with resume;
  - whether Veritas encrypts the files end to end itself.
- **Direct pairing (decision-4, VRT-26.1/26.2) stays as the second route.** It is the route for users without Nextcloud.

## Consequences

- **Local-first rule (AGENTS.md):** the user's own Nextcloud is now an allowed destination. It is off by default, explicitly connected, and revocable, and it never receives voice embeddings or the database. Third-party cloud services and telemetry remain out.
- **Tickets:**
  - VRT-37: transcript as Markdown next to the audio. Useful on its own, since a meetings folder becomes readable without the app.
  - VRT-38: the Nextcloud spike.
  - VRT-39: desktop sync.
  - VRT-26.3 and VRT-26.4 now cover upload to the inbox, reading `transcript.md`, and search on the phone.
- The phone builds its own search index from the Markdown files. The desktop's FTS index stays desktop-only.
- **Tests:** sync is tested against a local Nextcloud container, never a real account.
