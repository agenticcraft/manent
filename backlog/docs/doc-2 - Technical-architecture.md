---
id: doc-2
title: Technical architecture
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-10-29T22:00:00Z'
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
**One model for everything, Dutch and English (decided in VRT-32):** `parakeet-cpp-bundle-standard` (1.1 GB, Q8_0). It holds parakeet-tdt 0.6B v3 (ASR, 25 European languages, detects the language itself), Nemotron-3 diarization, WeSpeaker speaker embeddings, Silero VAD and a sound tagger. It serves live captions, the rolling and final passes, diarization and speaker embeddings. No streaming models are needed.

- `parakeet-cpp` is a C++/ggml port of NVIDIA NeMo Parakeet. It runs quantized GGUF models on CPU at upstream accuracy and returns **word-level timestamps**.
- **Backend variant:** install the CPU backend explicitly (`cpu-parakeet-cpp`, 29 MB). LocalAI's auto-detection picks a GPU build when it sees one (for example the multi-GB ROCm build on AMD).
- **Language:** auto only. The `language` request field is ignored by parakeet 0.6B v3 (auto, `nl` and `en` give identical output), so the app has no language setting. Mixed Dutch/English meetings work because each chunk is detected on its own.
- **Measured** (test machine, CPU): RTF 0.10 for transcription, 0.03 for diarization. Offline WER on spontaneous English meetings 14–20% (AMI, VRT-31); Dutch 11% on read speech (FLEURS) and 8–17% disagreement with YouTube captions on Dutch interviews (VRT-32).
- **Rejected for live** (VRT-2, VRT-31, VRT-32): `realtime_eou_120m` streaming (English only; 28.9% WER on a real meeting, drops crosstalk, no live speaker segments) and `nemotron-3.5-asr-streaming-0.6b` (32% WER on mixed NL/EN, leaks language tags). LocalAI's realtime WebSocket with `server_vad` waits for pauses, so text lagged up to 34 s on fluent speech.
- **Licences** (NOTICE-parakeet-bundle-standard.txt): CC-BY-4.0 (ASR, WeSpeaker), OpenMDW-1.1 (diarization: permissive, keep the licence and notices when distributing), MIT (Silero VAD), Apache-2.0 per model card for the sound tagger (upstream licensing inconsistent; Veritas doesn't use it). The app shows the attributions; if the model is ever bundled, the NOTICE file ships with it.

How the app uses it:
- The app runs **LocalAI as a sidecar process**, bound to `127.0.0.1` on a random port, started with a raised `--upload-limit` (the default is 15 MB). Rust supervises it: start, health check, restart, and installing the CPU backend and the model with a progress bar. How the engine is packaged and delivered (bundled or downloaded after install, full LocalAI or something leaner) is decided in VRT-29. LocalAI only discovers newly installed backends at startup, so it restarts after a backend install.
- **Three passes per meeting:**
  1. **Live captions (app-side chunking, validated in VRT-32):** per track, the app cuts the 16 kHz audio at the quietest 20 ms frame between 4 and 8 s after the previous cut, so a chunk never waits for a pause, and POSTs each chunk (FLAC) to `/v1/audio/transcriptions`. Text appears a median 2.8–4 s and at most ~8 s after it is spoken, at ~0.7 s processing per chunk. Accuracy is close to offline (AMI meeting window 19.1% vs 15.1% offline; Dutch interview within 5.5% of offline). Captions are shown in light grey and carry no speaker.
  2. **Rolling pass:** every ~30–60 s the app sends the latest audio (with some overlap) to `/v1/audio/transcriptions` and `/v1/audio/diarization`. The resulting segments, with speaker labels and names, replace the captions for that stretch. At RTF ~0.1 + 0.03 this keeps up easily on CPU.
  3. **Final pass:** at the end of the meeting the whole recording is transcribed and diarized once more, so speaker labels are consistent across the meeting. This is the stored transcript.
- **Offline (final pass, recordings and imports):** `POST /v1/audio/transcriptions` with `response_format=verbose_json` and `timestamp_granularities[]=word` returns segments with a speaker label and nested words. `POST /v1/audio/diarization` with `include_speaker_profiles=true` adds per-speaker embeddings, which the app matches to names itself (§1.4). The app merges the two by timestamp. Don't use `include_text=true` on the diarization endpoint: in LocalAI v4.12.1 it collapses everything into one segment. Uploads are FLAC (identical results, 3.4× smaller than WAV on clean audio). Diarization almost never assigns speech to the wrong person (0.1–0.3% on AMI, 6.6% in one meeting with a very quiet speaker); produced audio often yields extra tiny speaker slots (< 8 s of clean speech).
- An `Engine` trait in Rust (`transcribe_chunk`, `transcribe_file`, `speaker_profiles`) hides LocalAI. That makes it possible later to swap in parakeet.cpp linked directly through FFI, which mobile will need because there is no sidecar there.

### 1.4 Speaker identity and naming
Validated in VRT-30 (LibriSpeech), VRT-31 (AMI, across sessions) and VRT-32 (Dutch interviews, across recordings and across Dutch/English): every enrolled person was named correctly and no unenrolled voice got a name.

- **Embeddings come from LocalAI, matching happens in the app.** `/v1/audio/diarization` with `include_speaker_profiles=true` returns a 256-dim WeSpeaker embedding (and `clean_duration`) per speaker. Veritas compares those with its own registry using cosine distance in Rust. It doesn't depend on LocalAI's voice registry, which is in memory (lost on restart) and needs the extra `local-store` backend.
- **Registry:** tables `speakers(id, name, color)` and `voice_samples(id, speaker_id, embedding BLOB, encoder_id, clean_seconds, meeting_id, created_at)`. A person has several samples. Matching uses the nearest sample, and only samples from the same encoder (`encoder_id` = the `sha256:` identity in `speaker_profiles`).
- **Decision rule** (cosine distance to the nearest sample of each person): **auto-name** when the nearest person is ≤ 0.40 *and* at least 0.15 closer than any other enrolled person; **suggest** the nearest name (user confirms) when it is ≤ 0.60; otherwise leave the speaker unnamed ("Speaker N"). Measured: same person 0.06–0.39 (worst case: 6 s enrollment on degraded audio); different people ≥ 0.46 (two Dutch men in the same studio), usually ≥ 0.68.
- **Tiny slots:** diarization slots with < 5 s of clean speech are never auto-named and never used as samples; the user can merge them into a named speaker.
- **Naming a speaker:** when the user clicks an unnamed "Speaker 2" chip and types "Anna", that speaker's embedding from this meeting becomes a sample for Anna. Every confirmed or corrected name adds a sample, so matching adapts to real mics and rooms over time. Only speakers with at least 5 s of clean speech are used as samples.
- **Named transcript:** transcription segments get the name (or "Speaker N") of the diarization segment they overlap most. Consecutive segments of the same speaker form one block.
- Renaming a speaker updates every segment retroactively. Merging speakers (for example "Speaker 1" and "Speaker 3" are the same person) moves their samples to one person. Deleting a person deletes all their samples.

### 1.5 Storage
- **SQLite** via `sqlx` (bundled SQLite, WAL, foreign keys on) in the app data folder: `<app data>/veritas.db`, opened and migrated at app start (`src-tauri/migrations/`, implemented in VRT-6). Times are milliseconds: absolute since the Unix epoch, or relative to the meeting start.
  - `meetings(id, title, started_at, duration_ms, status ∈ recording|processing|done|failed, dir)`
  - `tracks(id, meeting_id → meetings, source_kind ∈ mic|loopback, source_id, device_name, path, seconds)`
  - `segments(id, meeting_id → meetings, track_id → tracks, pass ∈ caption|rolling|final, speaker_id → speakers, speaker_label, start_ms, end_ms, text, words_json)`: the three passes of §1.3; a rolling or final pass atomically replaces the segments of earlier passes in its time window.
  - `speakers(id, name, color)` and `voice_samples(id, speaker_id → speakers, embedding BLOB (LE f32), encoder_id, clean_seconds, meeting_id → meetings, created_at)` (§1.4).
  - `settings(key, value JSON)`.
- Deleting a meeting cascades to its tracks and segments; deleting a person deletes their voice samples (segments stay, unnamed).
- **Search:** `segments_fts` (FTS5, `unicode61 remove_diacritics 2`, kept in sync by triggers). Each word is prefix-matched and all words must match; diacritics and case are ignored, so "ideeen" finds "ideeën".
- Each meeting has a folder `<app data>/meetings/<id>/` holding `mic.flac` and `system.flac` (one file per source, §1.2).

### 1.6 Project layout
```
veritas/
  src-tauri/
    src/
      main.rs            # thin entry point (calls lib::run)
      lib.rs             # tauri setup, module wiring, command registration
      commands.rs        # Tauri commands (kept in sync with src/lib/ipc.ts)
      audio/             # sources (linux.rs, windows.rs, macos.rs), ring buffer, resampler, recorder
      engine/            # Engine trait, localai.rs (sidecar + HTTP client)
      speakers.rs        # registry + in-app matching
      db.rs              # sqlx migrations + queries
    binaries/            # local-ai sidecar per target triple (VRT-14/VRT-29)
  src/                   # React UI
    routes/              # Library, Live, Meeting, Speakers, Settings
    components/          # Sidebar, PageHeader, EmptyState; later TranscriptView, SpeakerChip, LevelMeter, SourcePicker
    components/ui/       # shadcn/ui components
    lib/ipc.ts           # typed wrappers around invoke/listen
    lib/theme.ts         # follow the OS light/dark preference
    index.css            # design tokens (doc-3)
  scripts/ui-smoke.py    # UI smoke check of the real app via tauri-driver
```
