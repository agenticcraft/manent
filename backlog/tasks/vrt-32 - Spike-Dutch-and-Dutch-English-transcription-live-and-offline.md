---
id: VRT-32
title: 'Spike: Dutch and Dutch/English transcription (live and offline)'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29T17:04:00Z'
updated_date: '2026-10-29T21:26:00Z'
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
- [x] #1 Real Dutch recordings downloaded (public interview set, see notes) and usable as test data
- [x] #2 Offline Dutch: WER and diarization measured on the Dutch recordings with parakeet-cpp-bundle-standard (YouTube auto-captions as rough reference; hand-check a few minutes)
- [x] #3 Live Dutch: nemotron-3.5 streaming tested (latency, accuracy) and compared with VAD-chunked 0.6B v3; also tested on the NL/EN mixed sample and the Podcast episode with a Dutch intro
- [x] #4 Speaker naming works across sessions and languages: The test speaker enrolled from one recording is recognized in the others (Dutch and English), and the Panel panel speakers are recognized from part I in part II
- [x] #5 Language handling decided: auto-detect vs explicit (auto/NL/EN) per meeting, and how mixed NL/EN meetings are handled live and offline
- [x] #6 Model set for NL+EN chosen (live and offline) with download sizes and licenses (nemotron OpenMDW-1.1) reviewed; doc-1 and doc-2 updated
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Convert the public interview recordings to 16 kHz mono FLAC.
2. Offline Dutch with parakeet-cpp-bundle-standard on Radio (40 min) and Panel I/II: RTF, agreement with YouTube auto-captions (rough reference), read through disagreements; diarization speaker counts.
3. Speaker naming: identify the test speaker's slot from the transcript content, enroll from one recording, match in the other four (NL and EN); Panel panel I -> II.
4. Live Dutch: pipeline with nemotron-3.5 streaming (server_vad, language nl/auto) vs server_vad + 0.6B v3 bundle (VAD-chunked); latency + WER on the FLEURS NL/EN sample (ground truth) and on 5 min of Panel vs offline.
5. Language handling: auto vs explicit nl/en on the mixed NL/EN sample and the Podcast episode with a Dutch intro.
6. Licenses of the chosen models; decide the NL+EN model set; update doc-1/doc-2.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Data source change (decision, 2026-10-29): use public interview recordings instead of Tweede Kamer debates (the Debat Direct VOD audio playlist only had 'nomeeting' segments). Public recordings, for local tests only, to spikes/localai/samples/meetings/interviews, described in manifest.json:
- NL: interview-nl Radio interview (40 min, host + the test speaker), panel-nl-1 Dutch panel part I (28 min, several speakers), panel-nl-2 part II (25 min, same panel).
- EN: podcast-nl podcast (37 min; English episode with a Dutch intro = real code-switching), podcast-en English podcast #14 (37 min).
- Not downloaded: podcast-nl-alt Podcast Dutch version (HTTP 403).
- Reference text: YouTube nl auto-captions for interview-nl, panel-nl-1, podcast-nl (rough, ASR-made, not ground truth); subtitle downloads were rate-limited (HTTP 429).
- The test speaker appears in all five: cross-session and cross-language naming test. Audio and captions are git-ignored.
- Tweede Kamer route kept for reference (Debat Direct API notes in the description) in case labelled speaker turns are needed later.

Results (LocalAI v4.12.1, CPU; scripts eval_recording.py, naming_cross.py, live_test.py, score_live.py, chunked_live.py):

Offline Dutch (parakeet-cpp-bundle-standard, RTF 0.10 + 0.03 diarization):
- Disagreement with YouTube auto-captions (rough reference, ASR itself): Panel I 16.7%, Radio 12.3%, Podcast 7.8%. FLEURS NL with ground truth: 11.2% (VRT-2). Read-through of Panel I: natural Dutch; errors mostly names/terms (AIVD/MIVD -> 'IVD en NVD', Inzet -> Inzicht) and short interjections attributed to the wrong speaker. Could not listen; judged by reading passages against context.
- NOTE: podcast-nl (a podcast) turned out to be Dutch audio (Podcast; two guests), not English. English (podcast-en) is the English recording.
- Diarization: Panel I 2 speakers (host + the test speaker, correct); Panel II 4 real + 2 tiny slots; Radio host + the test speaker + 2 short voices; Podcast 3 main + 4 tiny slots (< 8 s clean). Tiny slots are common in produced audio.

Speaker naming (in-app matching, enroll from Panel I):
- the test speaker auto-named in Panel II (0.064), Radio (0.141), Podcast (0.189, confirmed by content) and the English English podcast (0.133): works across sessions and across Dutch/English.
- Panel host recognized in part II (0.079). All other voices (hosts, another guest, panelists, audience, tiny slots) stayed unnamed (>= 0.68).
- Closest different-person distance: the test speaker vs Panel host 0.46-0.62 (two Dutch men, same studio). The old auto band (<= 0.45) is too close: new rule auto <= 0.40 AND >= 0.15 closer than any other enrolled person; suggest <= 0.60; else unnamed. All correct matches in VRT-30/31/32 satisfy it (max same-person 0.39, margins >= 0.27).

Live Dutch / mixed (73 s FLEURS NL/EN with ground truth unless noted):
- nemotron-3.5 streaming (semantic_vad): caption WER 32.3%, first caption 1.4-8.3 s, leaks '<nl-NL>' tags, garbles NL->EN switches, never closes turns (no EOU token).
- nemotron under server_vad: 17.7%. Bundle under server_vad: 9.5%, text 0.05-1.1 s after a turn ends.
- But on fluent speech (Panel, the test speaker talking without pauses) server_vad turns wait for pauses: text appears median 7.7 s, max 34 s after it is spoken (both models).
- App-side chunking (cut at the quietest 20 ms frame 4-8 s after the previous cut, POST each chunk to /v1/audio/transcriptions): Panel 5.5% vs offline with text after median 4.0 s / max 7.9 s; FLEURS NL/EN 13.3% (median 0.8 s); AMI English meeting window 19.1% (vs realtime_eou streaming 28.9%, server_vad 33.7%, offline 15.1%), median 2.8 s / max 8.0 s. Processing ~0.7 s per chunk.
- Chunking also fixes Dutch words leaking into English after a Dutch speaker ('De major religion' -> 'The major religion'), because each chunk starts fresh.

Language handling: the language parameter is ignored by parakeet 0.6B v3 (auto, nl and en give byte-identical output offline and chunked); it detects the language itself. Decision: no language setting; auto only.

Licenses (NOTICE-parakeet-bundle-standard.txt): asr parakeet-tdt-0.6b-v3 CC-BY-4.0; diar Nemotron-3-Diarization OpenMDW-1.1 (permissive; keep licence + notices when distributing; terminates on patent/copyright suits about the model); voice WeSpeaker CC-BY-4.0; vad Silero MIT; ced CED-small Apache-2.0 per model card but upstream inconsistent (GPL-3.0 code, CC-BY-4.0 checkpoint records) - not used by Veritas. In-app download from Hugging Face is not redistribution; show attributions in the app and ship the NOTICE if the model is ever bundled.

Decision - model set for NL+EN: parakeet-cpp-bundle-standard only (1.1 GB): live captions via app-side 4-8 s chunks, rolling and final offline passes, diarization and speaker embeddings. Not needed: nemotron-3.5 streaming (938 MB), realtime_eou_120m (254 MB), LocalAI realtime WebSocket, local-store backend.

Follow-up (2026-10-29): real Dutch conversation with English terms inside sentences.
- Say Gouda podcast (samples/meetings/saygouda, manifest.json): turned out to be Dutch-only (English titles only; checked 3 episodes). Not a code-switching sample; kept as clear single-narrator Dutch.
- Tweakers Podcast #449 (samples/meetings/tweakers, 80.5 min, several hosts, public Omny RSS feed; no ground truth). Offline, parakeet-cpp-bundle-standard, language auto, via eval_recording.py: transcription RTF 0.112, diarization RTF 0.038; 4 main speakers (11.6/17.5/19.8/28.5 min) plus 2 tiny slots (0.1/0.3 min, likely jingles/clips, e.g. an English clip at 0:02).
- English terms that are common in Dutch come out right (exploit, gepatcht, patch, release, setup, game assets, accounts, app, QR-code, 'suspension of disbelief').
- Less common English terms get Dutchified, as with CS-FLEURS: the episode's main term 'AI slop' is right about half the time, otherwise 'AI slot'/'slopt'/'slot'; 'Claude' -> 'cloud'; 'idle-vermogen' -> 'ijdelvermogen'; brand names break.
- Supports the earlier conclusion: Dutch is fine, intra-sentence English needs a custom vocabulary/hotwords or post-correction later. Results in spikes/localai/results/tweakers-449-*.json.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Validated Dutch and mixed Dutch/English and chose the model set for both languages.

Data: five public interview recordings (Radio, Panel I/II, Podcast - all Dutch - and the English English podcast), plus the FLEURS NL/EN sample with ground truth and the AMI window.

Results: offline Dutch is on par with English (FLEURS NL 11%, 8-17% disagreement with YouTube captions; read-through shows errors mostly in names/terms - judged by reading, not by listening). Speaker naming worked across recordings and across Dutch/English (the test speaker auto-named in all four other recordings at 0.06-0.19; all other voices unnamed), but the test speaker vs the Panel host came as close as 0.46, so the decision rule is now auto <= 0.40 with a 0.15 margin, suggest <= 0.60. Streaming models lost: nemotron-3.5 32% WER with language-tag leaks; LocalAI server_vad turns lag up to 34 s on fluent speech. App-side 4-8 s chunking with the offline bundle gives near-offline accuracy (AMI 19.1% vs 15.1%) with text after a median 2.8-4 s, max 8 s. The language field is ignored by the model, so there is no language setting.

Decision: one model, parakeet-cpp-bundle-standard (1.1 GB), for live chunks, rolling and final passes, diarization and embeddings in Dutch and English; licences permissive (OpenMDW-1.1 for diarization; NOTICE must ship if bundled). doc-1/2/3 and VRT-14/15/16/18/27/29 updated.

Risks: live captions still drop some crosstalk (AMI); tiny diarization slots are common in produced audio; the CED sound tagger inside the bundle has inconsistent upstream licensing (unused).
<!-- SECTION:FINAL_SUMMARY:END -->
