---
id: VRT-2
title: 'Spike: LocalAI parakeet-cpp engine (English, streaming, diarization)'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T16:41:00Z'
labels:
  - spike
  - transcription
milestone: m-0
dependencies: []
references:
  - https://localai.io/docs/features/audio-to-text/
documentation:
  - doc-2
priority: high
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Validate LocalAI + parakeet-cpp on a laptop CPU for English meetings: offline transcription with word timestamps, diarization, speaker embeddings, and live streaming with a cache-aware streaming model (realtime_eou_120m, e.g. via parakeet-cpp-realtime-scene-speakers). Pin down the exact realtime and speaker-registration API for the installed LocalAI version. Multi-language (Dutch) is out of scope here and tracked in VRT-28. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 local-ai runs locally with parakeet-cpp-bundle-standard installed
- [x] #2 File transcription returns word timestamps (timestamp_granularities[]=word)
- [x] #3 Speaker-labelled segments are available for a multi-speaker recording
- [x] #4 English sample transcribes acceptably (Dutch/English results moved to VRT-28)
- [x] #5 Live English streaming tested with a cache-aware streaming model, with measured latency
- [x] #6 Real-time factor measured on CPU (target < 0.3)
- [x] #7 Realtime and speaker-registration endpoints/payloads documented in notes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Install LocalAI v4.12.1 (sha256-verified) into spikes/localai/bin; data in spikes/localai/data (git-ignored).
2. Install parakeet-cpp-bundle-standard and the CPU parakeet-cpp backend (not the auto-detected ROCm build).
3. Test offline transcription (word timestamps), diarization and speaker profiles with curl on FLEURS-based samples (build_samples.py, analyze.py).
4. Rescoped to English (decision); NL/EN results moved to VRT-28.
5. Live English: realtime WebSocket transcription session with realtime-scene-speakers + semantic_vad, streamed at real-time pace (live_test.py); measure latency.
6. Speaker naming: portable /v1/voice/register from speaker_profiles, check names on another recording, /v1/voice/forget.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
LocalAI v4.12.1 binary downloaded (GitHub throttled to ~20 KB/s per connection; aria2c -x16 got ~2.7 MB/s). Gallery has extra candidates worth testing: parakeet-cpp-nemotron-3.5-asr-streaming-0.6b (streaming 0.6B, check languages) and parakeet-cpp-multilingual-diarization-speakers.

Setup: LocalAI picked the ROCm backend (multi-GB) because of the Radeon 890M via meta-backend capabilities (amd: rocm-parakeet-cpp). Installed localai@cpu-parakeet-cpp explicitly (29 MB). The app must install the cpu-* backend variant explicitly (or force the capability) instead of relying on auto-detection.
The spike runs on 127.0.0.1:8181 (app: random free port, as planned).
Test data (spikes/localai/samples, built by build_samples.py): meeting-nl-en.wav (73 s, 8 alternating NL/EN turns from FLEURS, 4 pitch-picked voices, truth JSON) + 3 CS-FLEURS nld-eng clips (synthetic TTS, intra-sentence switching).
Results /v1/audio/transcriptions (verbose_json, word timestamps), --threads 12:
- RTF: cold 10.08 s / 73.4 s = 0.14 (incl. model load); warm 7.43 s = 0.10.
- Meeting WER 11.3% (NL 11.2%, EN 11.5%), strict normalisation (compounds/hyphens count). Main real errors: Dutch function words leaking into English sentences (the->De, and->en, for->voor, Africa->Afrika).
- Diarization within the transcription response: per-segment 'speaker' labels; both EN voices separated (1, 2) consistently, both NL voices merged into 0 (FLEURS NL may be the same speaker; unverified).
- Intra-sentence code-switching (CS-FLEURS, synthetic spliced TTS): WER 37-50%, English words inside Dutch sentences get Dutchified. Known weakness; between-sentence/speaker switching is fine.
- Response shape: {text, segments:[{id,start,end,text,tokens,speaker,words:[{start,end,text}]}], words:[...]}; no language field.

Scope change (decision, 2026-10-29): English only for now. Multi-language support, incl. the Dutch/English results above and the nemotron-3.5 streaming candidate, moved to VRT-28.

Downloads (2026-10-29): streaming setup fetched with aria2c + sha256 (HF install via local-ai stalled at 0 bytes): realtime_eou_120m-v1-f16 (254 MB), nemotron-3-diarization-q8_0 (104 MB), ced-tiny-q8_0 (6 MB), voice-detect-wespeaker-resnet34 (25 MB), plus nemotron-3.5-asr-streaming-0.6b-q8_0 (938 MB, for VRT-28). Model YAMLs written by hand from the gallery overrides.

Live English streaming (realtime WebSocket, spikes/localai/live_test.py, samples/meeting-en.wav = 4 EN FLEURS turns, 41 s, paced at real time in 100 ms PCM16 16 kHz chunks):
- Protocol: ws://127.0.0.1:8181/v1/realtime?model=<pipeline>; session.update {session:{type:'transcription', audio:{input:{format:{type:'audio/pcm',rate:16000}, transcription:{model:<pipeline>,language:'en'}, turn_detection:{type:'semantic_vad',eagerness:'medium'}}}}}; then input_audio_buffer.append {audio:<base64 PCM16>}.
- The session model must be a pipeline config with pipeline.vad + pipeline.transcription (no LLM/TTS needed). Used data/models/veritas-live-en.yaml: vad = parakeet-cpp-bundle-standard (its VAD head), transcription = parakeet-cpp-realtime-scene-speakers, semantic_vad.
- Events: input_audio_buffer.speech_started/speech_stopped/committed, conversation.item.input_audio_transcription.delta (live captions), ...completed (final text per turn).
- Latency: words arrive every ~0.3 s while speaking; first caption 1.1-2.2 s after speech start (3.1 s on the first turn incl. warm-up); final transcript 0.7-1.2 s after the turn ends.
- Accuracy: 4/4 turns near-perfect (only aps->eps), better than offline file streaming (elephants 'and' giraffes correct).
- Live speaker segments (conversation.item.input_audio_transcription.segment): NOT emitted in this test, also with diarization_latency:ultra_low. Each turn had one speaker, so the diarization segment may not close before commit. Unresolved; speakers are available per segment from /v1/audio/transcriptions and per committed turn via pipeline.diarization (server_vad + offline model).

Speaker naming (/v1/voice/*):
- Needs the local-store backend (cpu-local-store, 16 MB) for the vector store; the server must be restarted after installing a backend.
- Portable registration works: POST /v1/voice/register {model:'parakeet-cpp-bundle-standard', name, labels, speaker_slot:<int>, speaker_profiles:<object from /v1/audio/diarization include_speaker_profiles=true>} -> {id,name,registered_at}. This maps directly onto the planned UX (click a speaker chip in a diarized meeting, type a name).
- After registering Alex/Bea from meeting-en, /v1/audio/diarization on meeting-nl-en named every EN segment correctly (name_score 0.995/0.998) and left the Dutch voices unnamed. Caveat: the EN clips are identical audio in both files, so this proves the mechanism, not generalisation to new speech.
- Names appear only on /v1/audio/diarization segments (name, name_score); /v1/audio/transcriptions only has speaker labels, so the app has to merge them by time.
- POST /v1/voice/forget {id} -> 204, unknown id -> 404; names disappear from the next diarization.
- The registry is in-memory and global: lost on LocalAI restart. Veritas must keep its own registry (store the speaker_profiles slot/embedding) and re-register on engine start, as planned in doc-2 §1.4.
- Not tested: /v1/voice/identify (a bundle doesn't serve the VoiceEmbed RPC; it needs a voice-detect model + backend).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Validated LocalAI v4.12.1 + parakeet-cpp on a laptop CPU (spike in spikes/localai: build_samples.py, analyze.py, live_test.py, wavio.py).

Results:
- Offline (parakeet-cpp-bundle-standard, 0.6B v3): RTF 0.10 warm, 0.14 cold; word timestamps and per-segment speaker labels; diarization found the right speakers and timeline; 256-dim speaker embeddings via include_speaker_profiles.
- Live English (realtime WebSocket transcription session, realtime_eou_120m + semantic_vad): captions every ~0.3 s, first caption 1.1-2.2 s after speech starts, final text 0.7-1.2 s after the turn ends, near-perfect accuracy on 4 turns.
- Speaker naming: portable /v1/voice/register from speaker_profiles named every English segment correctly on another recording; /v1/voice/forget removes it. Needs the cpu-local-store backend (16 MB); the registry is in-memory, so Veritas keeps its own and re-registers at startup.
- doc-2 §1.3/§1.4 updated with the verified protocol and endpoints.

Risks and follow-ups:
- Live speaker segments were not emitted (also with diarization_latency:ultra_low); speakers come from offline passes for now. Worth re-testing with real multi-speaker turns in VRT-17.
- Name matching was only proven on identical audio, not new speech from the same person.
- LocalAI auto-selects multi-GB GPU backends (ROCm), HF installs can stall, and include_text on diarization is broken in v4.12.1.
- Dutch/multilingual: VRT-28 (nemotron-3.5 streaming downloaded, not tested). Installer size and engine packaging: VRT-29.
<!-- SECTION:FINAL_SUMMARY:END -->
