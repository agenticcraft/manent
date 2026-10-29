---
id: VRT-28
title: Multi-language support (Dutch + English) for live and offline transcription
status: To Do
assignee: []
created_date: '2026-10-29T15:34:00Z'
updated_date: '2026-10-29T17:04:00Z'
labels:
  - transcription
  - i18n
milestone: m-4
dependencies:
  - VRT-16
references:
  - https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b
  - https://huggingface.co/mudler/parakeet-cpp-gguf
documentation:
  - doc-2
priority: medium
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Veritas starts English-only. This ticket adds multi-language support, first Dutch + English mixed meetings, for live and offline transcription.

Findings from the VRT-2 spike (LocalAI v4.12.1, parakeet-cpp CPU backend):
- parakeet-cpp-bundle-standard (parakeet-tdt 0.6B v3, 25 European languages) works well offline: RTF 0.10 on the test machine, 11.3% WER on a 73 s alternating NL/EN FLEURS meeting (NL 11.2%, EN 11.5%). It is NOT a cache-aware streaming model, so it can't stream live.
- Intra-sentence code-switching (English words inside Dutch sentences, CS-FLEURS synthetic clips) is weak: 37-50% WER.
- Candidate for live multilingual streaming: localai@parakeet-cpp-nemotron-3.5-asr-streaming-0.6b (nvidia/nemotron-3.5-asr-streaming-0.6b). Multilingual (40+ locales), prompt-conditioned, cache-aware streaming FastConformer RNN-T, about 2.5x faster than NeMo on CPU. The language is selected via the request 'language' field or auto-detected. GGUF in mudler/parakeet-cpp-gguf: q8_0 938 MB (gallery default), q6_k 816 MB, q5_k 748 MB, q4_k 685 MB, f16 1.4 GB. License OpenMDW-1.1 (check it is compatible with distribution).
- Fallback if nemotron streaming isn't good enough: VAD-chunked pseudo-streaming with the 0.6B v3 bundle (cut at pauses, send 2-6 s chunks; ~1-3 s latency given RTF 0.10).
- Test assets: spikes/localai/samples (meeting-nl-en.wav + truth JSON, cs-fleurs clips), rebuilt with spikes/localai/build_samples.py; scoring with spikes/localai/analyze.py.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Nemotron-3.5 streaming evaluated on the NL/EN meeting sample: WER and live latency measured and compared with VAD-chunked 0.6B v3
- [ ] #2 Live transcription works for Dutch and English, including switching between speakers
- [ ] #3 Offline transcription uses a multilingual model (0.6B v3 bundle or better)
- [ ] #4 Language setting (auto, EN, NL) available in Settings > Transcription and passed to the engine
- [ ] #5 Model license reviewed for redistribution in the app
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Superseded (2026-10-29): Dutch + English is now a core requirement, not a backlog item. Findings and the spike work moved to VRT-32 (Dutch spike); the implementation criteria moved into VRT-15 (language parameter), VRT-16 (live NL+EN), VRT-19 (offline NL+EN) and VRT-27 (language setting).
<!-- SECTION:NOTES:END -->
