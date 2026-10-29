---
id: VRT-31
title: 'Spike: English on real meeting recordings (AMI)'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29T17:04:00Z'
updated_date: '2026-10-29T17:54:00Z'
labels:
  - spike
  - transcription
  - speakers
milestone: m-0
dependencies:
  - VRT-30
references:
  - https://huggingface.co/datasets/FluidInference/ami-corpus-mirror
documentation:
  - doc-2
priority: high
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
VRT-2 and VRT-30 used read speech (FLEURS, LibriSpeech). Validate English on real, spontaneous multi-party meetings before building: transcription accuracy, diarization, speaker naming across sessions, and live streaming with real multi-speaker turns (live speaker segments were not emitted in VRT-2).

Data: AMI Meeting Corpus (CC-BY-4.0, mirror huggingface.co/datasets/FluidInference/ami-corpus-mirror), headset mix (Mix-Headset.wav) plus the official annotations (annotations/ami_public_manual_1.6.2.zip: per-speaker segments and word transcripts). Downloaded to spikes/localai/samples/meetings/ami: ES2004a (17 min, 4 speakers, scenario meeting) and IS1009a (14 min). The ES2004 series (a-d) has the same 4 people in later meetings, so ES2004b is the cross-session naming test.

Spike code in spikes/localai (analyze.py, eval_naming.py, live_test.py). See doc-2 Technical architecture §1.3, §1.4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Offline WER measured on AMI meetings against the official word transcripts
- [x] #2 Diarization compared with the AMI speaker segments (speaker count, share of speech attributed to the right speaker)
- [x] #3 Speaker naming tested across sessions: enroll from ES2004a, recognize the same people in ES2004b; decision bands from doc-2 §1.4 re-checked
- [x] #4 Live streaming tested on meeting audio: latency, accuracy and whether live speaker segments appear with real multi-speaker turns
- [x] #5 Findings recorded in notes; doc-2 updated where results change the design
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Download AMI ES2004b (same 4 participants as ES2004a) next to ES2004a/IS1009a.
2. Parse ami_public_manual_1.6.2 (words + segments per speaker, participant mapping) into a reference transcript and speaker turns per meeting (ami_reference.py).
3. Offline: transcribe ES2004a/IS1009a with parakeet-cpp-bundle-standard; WER vs AMI words (fillers removed); RTF on a 15-20 min file.
4. Diarization: speaker count and share of speech attributed to the right speaker (hyp label mapped to ref speaker by overlap).
5. Naming across sessions: speaker profiles from ES2004a vs ES2004b, cosine distances, doc-2 bands -> correct/suggest/wrong/unnamed.
6. Live: stream a few minutes of ES2004a in real time; latency, accuracy, and whether live speaker segments appear with real multi-speaker turns.
7. Record findings; update doc-2 where results change the design.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Data available: AMI ES2004a + IS1009a + annotations (samples/meetings/ami). Extra English test material from the public interview set (samples/meetings/interviews): English podcast #14 (podcast-en, 37 min) and podcast (podcast-nl, 37 min, Dutch intro). Still to download: AMI ES2004b for the cross-session naming test.

Results (LocalAI v4.12.1, parakeet-cpp-bundle-standard offline, veritas-live-en live; scripts ami_reference.py, eval_ami.py, naming_cross.py, live_test.py):

Offline (AMI headset mix, WER with fillers removed on both sides):
- ES2004a 17.5 min: WER 16.3%; RTF transcription 0.101, diarization 0.031.
- ES2004b 39.1 min: WER 14.4%.
- IS1009a 14.0 min: WER 19.6%.
- Most errors are deletions of short utterances/backchannels (hyp 7-12% fewer words than ref).

Diarization (10 ms frames of single-speaker reference speech, label mapped to the person it overlaps most):
- ES2004a: 4/4 speakers, right person 77.6%, wrong person 0.3%, missed 22.1%.
- ES2004b: 5 labels for 4 people (FEE013 split into 30 s + 3 s clean slots), right 81.9%, wrong 0.1%, missed 17.9%.
- IS1009a: 4/4, right 76.8%, wrong 6.6%, missed 16.7%; FIO087 (0.7 min of speech) mostly assigned to others (8.6% right). Rare/quiet speakers are the weak spot.
- 'Missed' is mostly short backchannels and word edges the diarizer doesn't mark as speech.

Cross-session naming (in-app cosine matching, doc-2 §1.4 bands, enroll ES2004a -> test ES2004b one hour later):
- 4/4 main slots auto-named correctly, distance 0.081-0.096 (same room/mics); next-nearest >= 0.70.
- The 3 s split slot of FEE013: 0.635 -> unnamed (missed). Short slots should be left for the user to merge.
- IS1009a strangers vs ES2004a: all 4 unnamed, nearest 0.682 (above the 0.60 suggest limit). Bands hold.

Live (5 min window ES2004a 120-420 s, streamed in real time):
- WER of completed turns 28.9% vs offline 15.1% on the same window (hyp 383 vs 443 vs ref 498 words): live drops much more speech, especially crosstalk (one 11 s stretch with 4 speakers and 55 words got no captions).
- Captions: median every 0.30 s while active; final transcript 0.05-4.9 s after the last word.
- semantic_vad 'turns' are 5-95 s long and contain 2-4 speakers; live speaker segments: 0 again (also with real multi-speaker audio). Live gives no usable speaker info.
- Conclusion: live is a fast preview; the authoritative transcript with speakers comes from offline passes. doc-2 §1.3 updated with a two-pass design (live captions + rolling offline re-transcription/diarization of recent audio + full pass at the end).

Other:
- LocalAI's default upload limit is 15 MB (--upload-limit); 17.5 min of 16 kHz WAV is 33 MB. FLAC uploads give identical transcripts at 3.4x smaller (1 h ~ 34 MB), so the app sends FLAC and starts the sidecar with a higher --upload-limit.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Validated English on real AMI meetings (ES2004a/b, IS1009a; spikes/localai: ami_reference.py, eval_ami.py, naming_cross.py, live_test.py).

Offline is solid: WER 14.4-19.6% on spontaneous speech at RTF ~0.1 (+0.03 diarization); diarization finds the right number of speakers and almost never assigns speech to the wrong person (0.1-0.3%; 6.6% in one meeting with a very quiet speaker). Cross-session naming with in-app matching: 4/4 people auto-named an hour later (distance <= 0.10), strangers stay unnamed (>= 0.68); doc-2 bands confirmed.

Live is a preview only: 28.9% WER vs 15.1% offline on the same 5 minutes, crosstalk dropped, turns span several speakers, no live speaker segments. doc-2 §1.3 now specifies a two-pass transcript (live captions -> rolling offline pass every 30-60 s -> final pass), uploads as FLAC with a raised --upload-limit (default 15 MB). VRT-17 updated to match.

Follow-ups: tiny diarization slots (< 5 s clean) should be left for the user to merge; quiet speakers can be misattributed.
<!-- SECTION:FINAL_SUMMARY:END -->
