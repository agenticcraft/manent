---
id: doc-2
title: Technical architecture
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-10-29T17:05:00Z'
---
## 1. Technical foundation

### 1.1 App shell: Tauri 2 (Rust core with a TypeScript UI)
- **Why Tauri:** one codebase covers Linux, macOS and Windows, and Tauri 2 also targets iOS and Android for the later mobile step. Installers are small (about 10 MB compared with about 150 MB for Electron). Rust gives low-latency, safe audio threads.
- **Frontend:** React, TypeScript, Vite, Tailwind CSS and shadcn/ui.
- **Backend (Rust):** audio capture, encoding, storage, and managing the transcription engine. It talks to the UI through Tauri commands, which carry requests, and events, which carry streams such as audio levels and transcript segments.

### 1.2 Audio capture layer (Rust module `audio/`)
Each **source** is either a mic input or a system-output loopback, and each one is captured as its own track. **One crate, `cpal` (0.18 or newer), covers both kinds on every desktop OS** (validated on Linux in VRT-1):

| OS | Mic | System audio (loopback) |
|---|---|---|
| Linux | `cpal` PipeWire host (feature `pipewire`) | `cpal` PipeWire host: an input stream on an output (sink) device sets `stream.capture.sink`, which is loopback |
| Windows | `cpal` WASAPI | `cpal` WASAPI loopback: an input stream on an output device |
| macOS | `cpal` CoreAudio | `cpal` CoreAudio loopback (an input stream on an output device, using an aggregate device). Verify the permissions and the minimum macOS version in VRT-12 |
| Android (later) | AudioRecord | `AudioPlaybackCapture` (MediaProjection), which only works for apps that allow capture |
| iOS (later) | AVAudioEngine | Not possible because of OS limits, so mic only |

Design:
- A `trait AudioSource { fn start(tx) ; fn stop() }` with one implementation per platform, chosen with `cfg`. Device enumeration returns a `SourceInfo {id, name, kind: Mic|Loopback}`.
- A lock-free ring buffer (`rtrb`) on each source, so the realtime callback never blocks.
- Two consumers for each source:
  1. **Recorder:** writes each track to its own file at the native sample rate. It writes **FLAC** while recording and can export to Opus or M4A afterwards. An optional mixdown file is used for playback.
  2. **ASR feed:** resamples to **16 kHz mono f32** (`rubato`) and sends 20–40 ms chunks to the transcription engine.
- Live level meters (RMS and peak) are sent to the UI at about 20 Hz.
- **Track alignment:** streams start a few ms apart (about 20 ms measured), so each track stores the timestamp of its first callback (`InputCallbackInfo`) to align the tracks. Over 5 minutes, VRT-1 measured no clock drift between mic and system tracks.
- **Key design advantage:** because the mic and system audio are separate tracks, the app already knows that speech on the mic track is the **local user ("Me")**. Diarization therefore only has to separate the remote voices on the system track, which makes the speaker labels much more accurate. Echo and bleed, where the mic picks up the speakers, are handled by preferring the system-track label when two segments overlap. Headphones avoid the problem entirely.

### 1.3 Transcription engine: LocalAI with the `parakeet-cpp` backend
From the LocalAI docs (localai.io/docs/features/audio-to-text):
- `parakeet-cpp` is a C++/ggml port of NVIDIA NeMo Parakeet. It runs quantized GGUF models on CPU at upstream accuracy and returns **word-level timestamps**.
- The same backend handles **cache-aware streaming** (the `realtime_eou_120m` model, which emits end-of-utterance events), **diarization** (`/v1/audio/diarization`, Nemotron-3 diarization model), **speaker naming** (`speaker_model`, WeSpeaker embeddings, `speaker_threshold` and `speaker_margin`), and **VAD** (`vad:true` or Silero).
- **Bundles** package all of these in one file:
  - `parakeet-cpp-bundle-standard` (1.1 GB): ASR with the **0.6B v3** model (25 European languages), plus diarization, sound events, speaker naming and VAD. **This is the offline default.** It is not a streaming model, so it can't do live transcription.
  - `parakeet-cpp-bundle-small` (338 MB): the same components with the 110M ASR model, which is English-only. It is an optional low-power fallback.
  - **Live (English):** a cache-aware streaming model, `realtime_eou_120m` (English, 254 MB), for example as `parakeet-cpp-realtime-scene-speakers`, which adds live diarization and speaker names. 
  - **Live (Dutch):** decided in VRT-32. Candidate: `nemotron-3.5-asr-streaming-0.6b` (multilingual cache-aware streaming, 938 MB q8_0, language per request or auto-detect). Fallback: VAD-chunked transcription with the 0.6B v3 bundle (~1–3 s latency).
  - **Backend variant:** install the CPU backend explicitly (`cpu-parakeet-cpp`, 29 MB). LocalAI's auto-detection picks a GPU build when it sees one (for example the multi-GB ROCm build on AMD).
- Since LocalAI 4.11, realtime sessions emit `conversation.item.input_audio_transcription.segment` events with `speaker` / `speaker_name`.

How the app uses it:
- The app runs **LocalAI as a sidecar process**, bound to `127.0.0.1` on a random port. Rust supervises it: start, health check, restart, and installing the CPU backends (`cpu-parakeet-cpp`, and models with a progress bar. How the engine is packaged and delivered (bundled or downloaded after install, full LocalAI or something leaner) is decided in VRT-29. LocalAI only discovers newly installed backends at startup, so it restarts after a backend install.
- **Live:** the app opens one realtime WebSocket *transcription* session for each source (validated in VRT-2):
  - Connect to `ws://127.0.0.1:<port>/v1/realtime?model=<pipeline>`. The pipeline is a model config with only `pipeline.vad` and `pipeline.transcription` (no LLM or TTS), using `semantic_vad` so the streaming model's end-of-utterance token closes turns.
  - Send `session.update` with `session.type = "transcription"`, `audio.input.format = {type: "audio/pcm", rate: 16000}`, the transcription model and `turn_detection`. Then stream `input_audio_buffer.append` with base64 PCM16 in 100 ms chunks.
  - Receive `conversation.item.input_audio_transcription.delta` (live captions, words about every 0.3 s) and `...completed` (final text per turn, about 1 s after the speaker stops).
  - Live speaker segments (`...segment` events) did not appear in VRT-2, so speakers come from an offline pass per committed turn or per meeting until that is solved.
- **Offline (recordings and imports):** `POST /v1/audio/transcriptions` with `response_format=verbose_json` and `timestamp_granularities[]=word` returns segments with a speaker label and nested words. `POST /v1/audio/diarization` with `include_speaker_profiles=true` adds per-speaker embeddings, which the app matches to names itself (§1.4). The app merges the two by timestamp. Don't use `include_text=true` on the diarization endpoint: in LocalAI v4.12.1 it collapses everything into one segment.
- An `Engine` trait in Rust (`transcribe_stream`, `transcribe_file`, `enroll_speaker`) hides LocalAI. That makes it possible later to swap in parakeet.cpp linked directly through FFI, which mobile will need because there is no sidecar there.

### 1.4 Speaker identity and naming
Validated in VRT-30: 10/10 turns named correctly on new speech, including 6 s enrollments and call-quality audio, with no false names on unenrolled speakers.

- **Embeddings come from LocalAI, matching happens in the app.** `/v1/audio/diarization` with `include_speaker_profiles=true` returns a 256-dim WeSpeaker embedding (and `clean_duration`) per speaker. Veritas compares those with its own registry using cosine distance in Rust. It doesn't depend on LocalAI's voice registry, which is in memory (lost on restart) and needs the extra `local-store` backend.
- **Registry:** tables `speakers(id, name, color)` and `voice_samples(id, speaker_id, embedding BLOB, encoder_id, clean_seconds, meeting_id, created_at)`. A person has several samples. Matching uses the nearest sample, and only samples from the same encoder (`encoder_id` = the `sha256:` identity in `speaker_profiles`).
- **Decision bands** (cosine distance to the nearest sample): ≤ 0.45 names the speaker automatically; 0.45–0.60 suggests the name for the user to confirm; > 0.60 leaves the speaker unnamed ("Speaker N"). Measured: same person 0.03–0.39, different people ≥ 0.76. Re-check the bands on real meetings.
- **Naming a speaker:** when the user clicks an unnamed "Speaker 2" chip and types "Anna", that speaker's embedding from this meeting becomes a sample for Anna. Every confirmed or corrected name adds a sample, so matching adapts to real mics and rooms over time. Only speakers with at least 5 s of clean speech are used as samples.
- **Named transcript:** transcription segments get the name (or "Speaker N") of the diarization segment they overlap most. Consecutive segments of the same speaker form one block.
- Renaming a speaker updates every segment retroactively. Merging speakers (for example "Speaker 1" and "Speaker 3" are the same person) moves their samples to one person. Deleting a person deletes all their samples.

### 1.5 Storage
- **SQLite** (`sqlx`, file-based) in the OS app-data folder.
  - `meetings(id, title, started_at, duration, status)`
  - `tracks(id, meeting_id, source_kind, device_name, path)`
  - `segments(id, meeting_id, track_id, speaker_id, start_ms, end_ms, text, words_json, is_final)`
  - `speakers(...)`
  - `settings(...)`
- Each meeting has a folder `meetings/<id>/` holding `mic.flac`, `system.flac` and `mix.opus`.
- An FTS5 index over segment text makes all meetings searchable.

### 1.6 Project layout
```
veritas/
  src-tauri/
    src/
      main.rs            # tauri setup, commands, events
      audio/             # sources (linux.rs, windows.rs, macos.rs), ring buffer, resampler, recorder
      engine/            # Engine trait, localai.rs (sidecar + HTTP/WS client)
      speakers.rs        # registry + enrollment
      db.rs              # sqlx migrations + queries
    binaries/            # local-ai sidecar per target triple
  src/                   # React UI
    routes/              # Library, Live, Meeting, Speakers, Settings
    components/          # TranscriptView, SpeakerChip, LevelMeter, SourcePicker
    lib/ipc.ts           # typed wrappers around invoke/listen
```
