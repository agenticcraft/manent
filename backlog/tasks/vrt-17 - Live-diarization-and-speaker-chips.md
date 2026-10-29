---
id: VRT-17
title: Live diarization and speaker chips
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T17:53:00Z'
labels:
  - transcription
  - frontend
milestone: m-2
dependencies:
  - VRT-16
documentation:
  - doc-2
priority: high
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Speaker labels via the two-pass design (doc-2 §1.3, validated in VRT-31): live captions carry no speaker info, so a rolling offline pass (every ~30-60 s, FLAC upload of recent audio to /v1/audio/transcriptions + /v1/audio/diarization) replaces captions with speaker-labelled segments, and a final pass at the end makes labels consistent across the meeting. The system track is diarized; the mic track is labelled 'Me' by default. Resolve mic bleed by preferring the system-track label on overlap. Render speaker blocks with colored name chips. See doc-2 Technical architecture §1.2, §1.3, §1.4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remote speakers get distinct labels (Speaker 1, 2, ...)
- [ ] #2 Mic speech is labelled 'Me' (renamable)
- [ ] #3 Overlap/bleed logic covered by unit tests
- [ ] #4 Current speaker block highlighted in the UI
- [ ] #5 Rolling pass replaces live captions with speaker-labelled segments within ~1 min; final pass relabels the whole meeting consistently
- [ ] #6 Verified with both a Dutch and an English multi-speaker recording
<!-- AC:END -->
