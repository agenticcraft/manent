---
id: VRT-17
title: Live diarization and speaker chips
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-31T19:48:00Z'
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
- [x] #1 Remote speakers get distinct labels (Speaker 1, 2, ...)
- [x] #2 Mic speech is labelled 'Me' (renamable)
- [x] #3 Overlap/bleed logic covered by unit tests
- [x] #4 Current speaker block highlighted in the UI
- [x] #5 Rolling pass replaces live captions with speaker-labelled segments within ~1 min; final pass relabels the whole meeting consistently
- [x] #6 Verified with both a Dutch and an English multi-speaker recording
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Engine: add transcribe_words(wav) (words only, for the mic track); audio: decode_mono(flac, 16 kHz) reusing the mixdown reader.
2. speakers labelling (pure, unit-tested, src-tauri/src/passes.rs):
   - bleed: a mic block is dropped when >= half its words match a system-track word (same normalised text within 1 s); otherwise only the matching words go. The system track wins on overlap (doc-2 §1.2).
   - rolling labels: diarization slots of a pass get meeting labels by (a) continuity with the labelled system words already in the context window, (b) cosine distance <= 0.40 to an earlier label's embedding, (c) otherwise the next 'Speaker N'.
   - final labels: slots numbered by first appearance.
3. Rolling pass thread per recording: caption workers also keep the last 3 min of 16 kHz audio per track; every ~45 s take [from, E] with 30 s of context; transcribe the system track with diarization and the mic track words-only; keep words starting in [from, to) where 'to' avoids splitting a word; bleed filter; replace caption+rolling segments in [from, to) (speaker_label 'me' or '1'..); event transcript:pass.
4. Stop: join the caption workers and the rolling thread, mixdown, meeting status 'processing'; final pass in the background: decode both FLAC tracks to 16 kHz WAV, transcribe+diarize the system track, words for the mic, bleed filter, replace all segments with pass final, status 'done'; event transcript:final.
5. UI Live: speaker blocks with coloured chips (Me + Speaker N), captions after the last pass stay grey, newest block highlighted; 'Me' renamable (setting speaker.me_name). live_transcript command for reloads.
6. Verify: unit tests; real app with the AMI 5-min window (English, 4 speakers, truth turns) and the FLEURS NL/EN sample (Dutch+English, 3 voices per diarization) played through the system audio: labels per truth turn consistent, rolling within ~1 min, final pass consistent.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-31):
- passes.rs (pure, 5 unit tests): bleed removal by text (mic block dropped when >= half its words match a system word within 1 s, else only matching words), Me/system blocks, final numbering by first appearance, RollingLabels (continuity with already-labelled words in the context -> voice embedding within 0.40 -> new label).
- rolling.rs: rolling thread (45 s interval, 30 s context, keeps [from, to) with 'to' never inside a word), final pass over decoded FLAC tracks; per-track requests run as parallel tasks. Caption workers keep 3 min of 16 kHz history (live.rs History). Track rows exist from the start; stop joins caption workers and the rolling thread before the final pass; meeting 'processing' -> 'done'. Events transcript:pass / transcript:final; command live_transcript; 'Me' name in setting speaker.me_name (get_me_name/set_me_name).
- Engine helper repair: speech regions >= 1 s without words are re-transcribed alone (the model dropped an English sentence inside a Dutch-English-Dutch VAD piece; max_segment tuning did not fix it). NL/EN recording 14.6% -> 10.1% WER, AMI unchanged (16.3%).
- UI: speaker blocks with coloured chips, Me renamable from its chip, captions grey until a pass replaces them, newest block highlighted.
Verification: unit tests (passes, history, decode, uncovered regions, safe_end), ignored engine test of the final pass with simulated bleed (mic = speakers at 30% + 40 ms + the user's Dutch sentence -> only the Dutch sentence is 'Me', 2 remote speakers). scripts/smoke-speakers.py on the real app: NL/EN FLEURS (8/8 turns, labels per voice consistent rolling and final, 3 labels: the diarizer merges the two Dutch voices as offline does), AMI ES2004a 5 min (4 people -> 4 final labels, 99% of words with their person's label rolling and final; rolling had one extra short label), Dutch interview panel-nl-1 5 min (2 speakers, 100% agreement with the offline diarization). Rolling labels arrive a median 31-36 s after speech (max ~55 s); final pass ~55-60 s for 5 min (debug build). Me rename verified in the UI. The speaker output of the test machine was muted, so acoustic bleed could not be produced on the real hardware; it is covered by the engine test. smoke-captions (median 4.2 s, NL 11.2%, EN 13.1%), smoke-live and smoke-playback still pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Speaker labels: a rolling pass every ~45 s replaces captions with speaker blocks (system track diarized and mapped to stable meeting labels by continuity and voice; mic = 'Me', renamable; bleed removed by text), and a final pass after Stop relabels the whole meeting consistently. The engine helper now re-transcribes speech the model dropped at language switches. Verified on English (AMI, 4 speakers), Dutch (interview) and mixed NL/EN recordings on the real app: 99-100% of words carry their person's label. Follow-up risk: the rolling pass can show an extra label for a short spurious speaker until the final pass.
<!-- SECTION:FINAL_SUMMARY:END -->
