---
id: VRT-10
title: Meeting playback with mixdown
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T08:13:00Z'
labels:
  - audio
  - frontend
milestone: m-1
dependencies:
  - VRT-5
  - VRT-7
documentation:
  - doc-2
priority: medium
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a mixed playback file (mix.opus) after recording and play it back in the app. See doc-2 Technical architecture §1.2, §1.5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 mix.opus created on Stop from all tracks
- [x] #2 Meeting can be played back with play/pause/seek
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Deps: opus 0.4 (libopus 1.6.1 built statically from bundled source; verified, no system lib), ogg (Ogg Opus container), claxon (FLAC decode, now a normal dependency), rubato (resample tracks that aren't 48 kHz).
2. audio/mixdown.rs: streaming mixdown (never loads whole tracks): decode each FLAC block-wise, downmix to mono, resample to 48 kHz if needed, sum (clamped), encode 20 ms Opus frames (VOIP, 32 kbps), write Ogg Opus per RFC 7845 (OpusHead with pre-skip, OpusTags, granule positions, EOS page). Unit tests: tracks at 48 kHz and 44.1 kHz with different tones -> decode the Ogg Opus with libopus: duration matches, both tones present, header fields valid.
3. stop_recording writes meetings/<id>/mix.opus after the tracks; failure of the mix doesn't lose the tracks.
4. Playback: get_meeting command (meeting + mix availability); Meeting page (from a Library row): title/date, player with play/pause, seek bar and time; audio via Tauri asset protocol (protocol-asset feature, scope allowed at runtime for <data>/meetings, CSP media-src).
5. Verify: Rust tests; smoke on the real app: record ~5 s with the test tone, stop -> mix.opus exists and decodes to the right length with the tone; open the meeting -> duration shown, play advances, pause stops, seek jumps; light/dark screenshots.

Revised (findings): WebKitGTK can't play media from Tauri's asset:// scheme (error 4 before any byte), and from a blob: URL it plays but can't seek (oggdemux in push mode: seeks snap to 0, duration only known from the current page). A probe with a Range-capable HTTP server on 127.0.0.1 gave the right duration and exact seeks. -> Serve mix.opus from a local media server (tiny_http, 127.0.0.1, random port, random 128-bit token in every URL, only <meetings>/<id>/mix.opus, Range support); CSP media-src http://127.0.0.1:*. Also: Ogg pages end every second (standard; players bisect on page granules).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation: audio/resample.rs (streaming mono rubato sinc resampler with delay compensation and exact output length; reused by VRT-16 for 16 kHz), audio/mixdown.rs (streaming FLAC -> mono 48 kHz -> Opus 32 kbps VOIP -> Ogg Opus per RFC 7845; pages every second; pre-skip from encoder lookahead; final granule trims padding), stop_recording writes mix.opus (a mix failure keeps the tracks), media_server.rs (tiny_http on 127.0.0.1:<random>, 128-bit token, only meetings/<id>/mix.opus, Range/HEAD, thread per request), get_meeting returns mix_url; UI: Library rows open src/routes/Meeting.tsx with src/components/AudioPlayer.tsx (play/pause, seek, time rounded down, preload auto). CSP media-src 'self' http://127.0.0.1:*. scripts/smoke-playback.py; doc-2 §1.5 and AGENTS.md updated.
Findings on the way (all reproduced and fixed):
1. asset:// playback fails in WebKitGTK (error 4 before reading any byte).
2. blob: URL plays but can't seek (seeks snap to 0; oggdemux in push mode).
3. One huge Ogg page (writer only closed pages when full) - fixed with a page per second (unit-tested), but not the root cause.
4. Probe with a Range HTTP server worked -> local media server.
5. The first media server version answered one request at a time; the player's parallel duration/seek requests queued behind the open stream -> thread per request (unit test with a slow reader).
6. preload='metadata' gives duration 0 / not seekable for Ogg in WebKitGTK even over HTTP; preload='auto' works (verified side by side with two servers).
7. Smoke tone check made robust: the mix carries room sound from the mic, so it compares the tone in the mix with the tone in system.flac.
Verification: 53 Rust tests (resampler 5, mixdown 4 incl. two rates + both tones + exact length + pages, media server 4 incl. traversal/token/ranges/concurrency); clippy 0; npm check clean. smoke-playback twice: mix.opus mono Opus, length = meeting, system tone present in the mix; player duration exact, play advances 1.45-1.48 s in 1.5 s, pause holds, seek to 3.5 s lands at 3.50 s. All other smoke scripts pass; OS default and real data untouched. Light screenshot of the player inspected.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Meetings can be played back: on Stop all tracks are mixed (streaming) into a small mono Ogg Opus file, and the Meeting page plays it with play/pause, seek and time. Getting reliable seeking in WebKitGTK took a local media server inside the app (127.0.0.1 only, random port, secret token per URL, only mix files, Range support, concurrent requests) plus preload=auto; Tauri's asset protocol and blob: URLs both failed in reproducible ways. Verified with unit tests and an end-to-end smoke check that records, mixes, plays, pauses and seeks in the real app.
<!-- SECTION:FINAL_SUMMARY:END -->
