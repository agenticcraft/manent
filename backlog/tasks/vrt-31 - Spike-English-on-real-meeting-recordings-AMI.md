---
id: VRT-31
title: 'Spike: English on real meeting recordings (AMI)'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-29T17:04:00Z'
updated_date: '2026-10-29T17:20:00Z'
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
- [ ] #1 Offline WER measured on AMI meetings against the official word transcripts
- [ ] #2 Diarization compared with the AMI speaker segments (speaker count, share of speech attributed to the right speaker)
- [ ] #3 Speaker naming tested across sessions: enroll from ES2004a, recognize the same people in ES2004b; decision bands from doc-2 §1.4 re-checked
- [ ] #4 Live streaming tested on meeting audio: latency, accuracy and whether live speaker segments appear with real multi-speaker turns
- [ ] #5 Findings recorded in notes; doc-2 updated where results change the design
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Data available: AMI ES2004a + IS1009a + annotations (samples/meetings/ami). Extra English test material from the public interview set (samples/meetings/interviews): English podcast #14 (podcast-en, 37 min) and podcast (podcast-nl, 37 min, Dutch intro). Still to download: AMI ES2004b for the cross-session naming test.
<!-- SECTION:NOTES:END -->
