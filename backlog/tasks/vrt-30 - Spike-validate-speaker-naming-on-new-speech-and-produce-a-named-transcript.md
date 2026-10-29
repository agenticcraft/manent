---
id: VRT-30
title: 'Spike: validate speaker naming on new speech and produce a named transcript'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29T16:49:00Z'
updated_date: '2026-10-29T16:54:00Z'
labels:
  - spike
  - transcription
  - speakers
milestone: m-0
dependencies:
  - VRT-2
documentation:
  - doc-2
priority: high
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
VRT-2 proved LocalAI speaker naming only on identical audio (the enrolled clips reappeared in the test file). Validate it properly and fix the naming flow so the app can show names in the transcript.

Approach: LibriSpeech test-clean (CC-BY-4.0, has speaker ids). Enroll several speakers from one set of utterances, then test on a meeting built from different utterances of the same speakers plus a speaker who was never enrolled. Tune the match threshold from the measured embedding distances. Merge diarization names into the transcription segments (/v1/audio/transcriptions has no names) to produce a named transcript. Spike code lives in spikes/localai. See doc-2 Technical architecture §1.4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Enrollment and test use different utterances of the same speakers, plus at least one unenrolled speaker
- [x] #2 Naming accuracy measured per segment: correct, wrong name, missed, false name on the unenrolled speaker
- [x] #3 Match threshold recommended from measured distances (same speaker vs different speaker)
- [x] #4 Named transcript produced by merging diarization names into transcription segments
- [x] #5 Findings and the recommended naming flow recorded in notes and doc-2 §1.4
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Build test data from LibriSpeech test-clean via the HF datasets API (build_speaker_samples.py): 4 enrolled speakers (Anna 2094, Ben 8230, Clara 4992, David 2300) enrolled from one set of utterances (22-33 s), a 110 s test meeting from different utterances (Clara from another chapter) plus 2 never-enrolled speakers (672, 3570).
2. eval_naming.py: enroll like the app (diarize with speaker profiles, portable /v1/voice/register of the dominant slot), diarize + transcribe the test meeting, score naming per turn, compute cosine distances from the profile embeddings, merge names into the transcription segments, forget the voices.
3. Stress tests: 6 s enrollment, degraded meeting audio (300-3400 Hz bandpass + pink noise, ~15 dB SNR), and both combined.
4. Derive the recommended naming flow and thresholds; update doc-2 §1.4.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Results (LocalAI v4.12.1, parakeet-cpp-bundle-standard, WeSpeaker 256-dim via speaker_profiles):
- Naming per turn: 10/10 in all four conditions (8 enrolled turns named correctly, 2 unenrolled turns left unnamed, no wrong or false names).
- Cosine distance test slot vs enrolled voice: clean full enrollment same<=0.206 / different>=0.778; 6 s enrollment 0.236 / 0.760; degraded full 0.321 / 0.805; degraded + 6 s 0.391 / 0.810. Different speakers never came below 0.76; same-speaker distance grows with degradation and channel/session change (worst: Clara, other chapter + 6 s + degraded = 0.391).
- LocalAI's default speaker_threshold 0.5 sits in the gap but the hardest case is within ~0.1 of it; real meetings (other days, other mics) are likely harder.
- Named transcript: merge by maximum time overlap between /v1/audio/transcriptions segments and /v1/audio/diarization segments works (results/named-transcript-*.txt); consecutive segments of the same speaker are joined into one block. Unnamed speakers show as 'Speaker N'.
Recommended naming flow (doc-2 §1.4 updated):
1. Match in the app, not in LocalAI's registry: take the per-speaker embeddings from include_speaker_profiles and compute cosine distances in Rust against the app's own SQLite registry. Removes the cpu-local-store backend (helps VRT-29), the in-memory registry that is lost on restart, and the re-registration step. LocalAI's own naming is then optional.
2. Several samples per person: every confirmed or corrected name adds that meeting's embedding as another sample; match against the nearest sample. Samples come from real meetings, so they cover the real mics/rooms and matching improves over time.
3. Three-band decision: distance <= 0.45 auto-name; 0.45-0.60 suggest the name (user confirms); > 0.60 unnamed. Re-check the bands on real meetings (needs real recordings, e.g. via spikes/audio-capture).
4. Only enroll from slots with enough clean speech (>= 5 s clean_duration in speaker_profiles; 6 s enrollment still worked here).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Validated speaker naming on new speech and redesigned the naming flow.

Test: LibriSpeech test-clean, 4 enrolled speakers (enrolled from other utterances, one from another chapter) + 2 never-enrolled speakers in a 110 s meeting (spikes/localai: build_speaker_samples.py, eval_naming.py). Naming was 10/10 per turn in all four conditions (clean or call-quality audio with noise, 20-30 s or 6 s enrollment), with no false names. Same-speaker cosine distance 0.03-0.39, different speakers >= 0.76; degradation and session change push same-speaker distance up, so real meetings need margin.

Changes: doc-2 §1.4 now specifies in-app matching (embeddings from include_speaker_profiles, SQLite registry with several samples per person, nearest-sample cosine matching, bands <= 0.45 auto / 0.45-0.60 suggest / > 0.60 unnamed) and the named-transcript merge by time overlap. That removes the dependency on LocalAI's in-memory voice registry and its local-store backend. VRT-18 updated to this design.

Follow-up: re-check the bands on real meeting recordings (other days, other mics); LibriSpeech is read speech.
<!-- SECTION:FINAL_SUMMARY:END -->
