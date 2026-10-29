---
id: VRT-2
title: 'Spike: LocalAI parakeet-cpp engine (NL/EN, streaming, diarization)'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - spike
  - transcription
milestone: m-0
dependencies: []
references:
  - 'https://localai.io/docs/features/audio-to-text/'
documentation:
  - doc-2
priority: high
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Validate LocalAI + parakeet-cpp-bundle-standard (parakeet 0.6B v3) on a laptop CPU for mixed Dutch/English meetings. Key risk: the cache-aware streaming model (realtime_eou_120m) is likely English-only, so verify whether 0.6B v3 can stream; otherwise prototype VAD-chunked pseudo-streaming. Also pin down the exact realtime and speaker-registration API for the installed LocalAI version. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 local-ai runs locally with parakeet-cpp-bundle-standard installed
- [ ] #2 File transcription returns word timestamps (timestamp_granularities[]=word)
- [ ] #3 /v1/audio/diarization with include_text=true returns speaker-labelled segments
- [ ] #4 Mixed Dutch/English sample transcribes acceptably
- [ ] #5 Live strategy decided: native streaming with 0.6B v3 or VAD-chunked fallback, with measured latency
- [ ] #6 Real-time factor measured on CPU (target < 0.3)
- [ ] #7 Realtime and speaker-registration endpoints/payloads documented in notes
<!-- AC:END -->
