---
id: VRT-32
title: 'Spike: Dutch and Dutch/English transcription (live and offline)'
status: To Do
assignee: []
created_date: '2026-10-29T17:04:00Z'
updated_date: '2026-10-29T17:20:00Z'
labels:
  - spike
  - transcription
  - i18n
milestone: m-0
dependencies:
  - VRT-31
references:
  - https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b
  - https://huggingface.co/mudler/parakeet-cpp-gguf
  - https://debatdirect.tweedekamer.nl
documentation:
  - doc-2
priority: high
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Veritas supports Dutch and English, including meetings that mix both. Validate Dutch the same way English was validated (VRT-2, VRT-30, VRT-31) and choose the model set for both languages.

Known so far (from VRT-2 / former VRT-28, LocalAI v4.12.1, parakeet-cpp CPU backend):
- Offline: parakeet-cpp-bundle-standard (parakeet-tdt 0.6B v3, 25 European languages) RTF 0.10 on the test machine; 11.3% WER on a 73 s alternating NL/EN FLEURS meeting (NL 11.2%, EN 11.5%). Main errors: Dutch function words leaking into English sentences (the->De, and->en, for->voor). Not a streaming model.
- Intra-sentence code-switching (English words inside Dutch sentences, CS-FLEURS synthetic clips): 37-50% WER, English words get Dutchified.
- Live candidate: parakeet-cpp-nemotron-3.5-asr-streaming-0.6b (nvidia/nemotron-3.5-asr-streaming-0.6b): multilingual (40+ locales), prompt-conditioned, cache-aware streaming FastConformer RNN-T, ~2.5x faster than NeMo on CPU; language via the request 'language' field or auto-detect. q8_0 (938 MB) is downloaded and configured in spikes/localai/data/models; also q6_k 816 MB, q5_k 748 MB, q4_k 685 MB, f16 1.4 GB. License OpenMDW-1.1.
- Fallback for live Dutch: VAD-chunked pseudo-streaming with the 0.6B v3 bundle (2-6 s chunks cut at pauses; ~1-3 s latency at RTF 0.10).
- English live is validated with realtime_eou_120m (English-only, semantic_vad end-of-utterance turns).

Real Dutch meeting data: Tweede Kamer via the public Debat Direct API (local tests only):
- GET https://cdn.debatdirect.tweedekamer.nl/api/agenda/<YYYY-MM-DD> lists debates (id, debateType, duration, locationId). Commissiedebat / Rondetafelgesprek are the most meeting-like.
- GET .../api/agenda/<date>/debates/<id> gives video.audioUrl (audio-only HLS, prog_index.m3u8) and events with timestamps: speaker / chairman / interrupter turns with politician ids, i.e. speaker ground truth for diarization and naming.
- The audio HLS for a room covers the whole day; cut the debate window (startedAt-endedAt) with ffmpeg. The playlist checked on 2026-10-29 for the previous day only had 11 'nomeeting' segments, so check availability/latency of the VOD audio first (video.vodUrl as alternative).
- Same politicians appear across debates: good for cross-session naming tests.

Existing test assets: spikes/localai/samples (meeting-nl-en.wav + truth, cs-fleurs), build_samples.py, analyze.py, live_test.py, eval_naming.py.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Real Dutch recordings downloaded (public interview set, see notes) and usable as test data
- [ ] #2 Offline Dutch: WER and diarization measured on the Dutch recordings with parakeet-cpp-bundle-standard (YouTube auto-captions as rough reference; hand-check a few minutes)
- [ ] #3 Live Dutch: nemotron-3.5 streaming tested (latency, accuracy) and compared with VAD-chunked 0.6B v3; also tested on the NL/EN mixed sample and the Podcast episode with a Dutch intro
- [ ] #4 Speaker naming works across sessions and languages: The test speaker enrolled from one recording is recognized in the others (Dutch and English), and the Panel panel speakers are recognized from part I in part II
- [ ] #5 Language handling decided: auto-detect vs explicit (auto/NL/EN) per meeting, and how mixed NL/EN meetings are handled live and offline
- [ ] #6 Model set for NL+EN chosen (live and offline) with download sizes and licenses (nemotron OpenMDW-1.1) reviewed; doc-1 and doc-2 updated
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Data source change (decision, 2026-10-29): use public interview recordings instead of Tweede Kamer debates (the Debat Direct VOD audio playlist only had 'nomeeting' segments). Public recordings, for local tests only, to spikes/localai/samples/meetings/interviews, described in manifest.json:
- NL: interview-nl Radio interview (40 min, host + the test speaker), panel-nl-1 Dutch panel part I (28 min, several speakers), panel-nl-2 part II (25 min, same panel).
- EN: podcast-nl podcast (37 min; English episode with a Dutch intro = real code-switching), podcast-en English podcast #14 (37 min).
- Not downloaded: podcast-nl-alt Podcast Dutch version (HTTP 403).
- Reference text: YouTube nl auto-captions for interview-nl, panel-nl-1, podcast-nl (rough, ASR-made, not ground truth); subtitle downloads were rate-limited (HTTP 429).
- The test speaker appears in all five: cross-session and cross-language naming test. Audio and captions are git-ignored.
- Tweede Kamer route kept for reference (Debat Direct API notes in the description) in case labelled speaker turns are needed later.
<!-- SECTION:NOTES:END -->
