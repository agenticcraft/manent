---
id: VRT-20
title: Meeting detail view with click-to-seek and search
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-31T21:15:00Z'
labels:
  - frontend
milestone: m-2
dependencies:
  - VRT-10
  - VRT-16
documentation:
  - doc-3
priority: medium
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Meeting page: transcript view plus waveform player; clicking a word seeks playback; global full-text search over all meetings via FTS5. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Clicking a word seeks audio to its timestamp
- [x] #2 Currently playing word/segment is highlighted
- [x] #3 Search finds meetings by transcript text and jumps to the match
- [x] #4 Search works for Dutch and English text, including diacritics (e.g. 'ë', 'é')
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-31):
- Player: waveform (600 bars from the FLAC tracks, cached waveform.json; one scaling SVG with the played part in the accent) under a transparent native range input (mouse/keyboard seeking unchanged, smoke-playback still passes); sticky at the bottom of the meeting screen; seek() handle that holds a position until WebKit can play (positions set while loading were dropped).
- Transcript (TranscriptBlocks): with onSeek every word is a span with its start; clicking a word or a block time plays from there. The playing word and its block are highlighted on the DOM (no re-render of every word), found by binary search over the words sorted by time (mic and system blocks interleave on a recording, so DOM order is not time order); the view follows playback unless the user scrolled away.
- Search: Library search also queries transcripts (search_transcripts, FTS5, 200 ms after typing); matches grouped per meeting with highlighted words (snippet markers changed from [ ] to U+0001/U+0002); a match opens the meeting there (player cued, block scrolled into view).
Verification: scripts/smoke-transcript.py on the real app, 3 passes in a row: 'technologieen' finds 'technologieën' (diacritics ignored), 'elephants' (English), 'toezichthouder' (Dutch); a match opens with the player at the match time and the block in view; waveform drawn, no horizontal overflow; clicking a word seeks there (within 0.3 s) and plays; the word being played and its block are highlighted. DB search tests (diacritics, prefix, all words) pass with the new markers; waveform unit test; smoke-playback passes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Meeting view: waveform player at the bottom, click any word (or block time) to play from there, the word and block being played are highlighted and followed; Library search covers all transcripts (Dutch and English, diacritics ignored) and opens a meeting at the match. Verified on the real app.
<!-- SECTION:FINAL_SUMMARY:END -->
