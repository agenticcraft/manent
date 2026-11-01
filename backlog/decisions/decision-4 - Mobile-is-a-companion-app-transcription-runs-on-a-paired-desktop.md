---
id: decision-4
title: Mobile is a companion app; transcription runs on a paired desktop
date: '2026-11-01T10:07:00Z'
status: accepted
---
## Context

The original plan (doc-1, VRT-26) was a mobile build of the same app, linking parakeet.cpp in-process. That would mean:
- a 1.1 GB model on the phone;
- per-platform engine builds;
- transcription draining the battery.

The desktop app already does the whole pipeline well: transcription, speakers, naming and search. It was decided on 2026-11-01 that the desktop versions stay the full app, and that mobile becomes a companion.

## Decision

- **Desktop** (Linux, Windows, macOS) is the full Veritas app.
- **Mobile** (Android, iOS) is a companion app that:
  - records on the phone;
  - is paired with an installed desktop Veritas;
  - delegates transcription to that desktop: it sends the recording and gets the transcript back.
- **No engine or model on the phone.** The privacy rule stays: audio, transcripts and voice embeddings go only between the user's own paired devices, never through a cloud service.
- **Pairing and transport** (discovery, pairing, encryption, and what happens when the desktop isn't reachable) are settled in a spike (VRT-26.1). That spike gets its own decision record before anything is built on it.

## Consequences

- The desktop gets a companion service that listens on the local network. It is off by default, accepts only paired devices, and can revoke them. This is the first network listener beyond `127.0.0.1`, so AGENTS.md's local-first rule now allows it only in this form.
- A recording from the phone becomes a meeting on the desktop through the existing import job (VRT-19), so known people are recognised as usual.
- The `Engine` trait no longer needs a mobile, in-process implementation.
- Live captions on the phone are out of scope for the first version. Recordings are transcribed after they reach the desktop. Streaming while on the same network is a possible later step.

