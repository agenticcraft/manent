---
id: doc-2
title: Technical architecture
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-10-29T14:27:00Z'
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
  - `parakeet-cpp-bundle-standard` (1.1 GB): ASR with the **0.6B v3** model, which covers 25 European languages including Dutch and English, plus diarization, sound events, speaker naming and VAD. **This is the default for both live and offline transcription.**
  - `parakeet-cpp-bundle-small` (338 MB): the same components with the 110M ASR model, which is English-only. It is an optional low-power fallback.
  - **Risk:** the cache-aware streaming model (`realtime_eou_120m`) appears to be English-only. For live Dutch, Phase 0 must check whether 0.6B v3 can stream. If it can't, the fallback is VAD-chunked pseudo-streaming: the app cuts the audio at pauses into 2–6 s chunks and sends each chunk to 0.6B v3. That gives a latency of about 1–3 s, which is fine on this CPU.
- Since LocalAI 4.11, realtime sessions emit `conversation.item.input_audio_transcription.segment` events with `speaker` / `speaker_name`.

How the app uses it:
- The app runs **LocalAI as a sidecar process** (the `local-ai` binary is bundled through Tauri `externalBin` and bound to `127.0.0.1` on a random port). Rust supervises it: start, health check, restart, and installing models on first run with a progress bar.
- **Live:** the app opens one OpenAI-style realtime/streaming session for each source (`/v1/realtime` WebSocket, or `/v1/audio/transcriptions` with `stream=true` as the fallback). It sends the 16 kHz chunks and receives text deltas and final segments with speaker labels.
- **Offline (recordings and imports):** `POST /v1/audio/diarization` with `include_text=true` and `timestamp_granularities[]=word`, using the standard bundle and `vad:true` for long files.
- An `Engine` trait in Rust (`transcribe_stream`, `transcribe_file`, `enroll_speaker`) hides LocalAI. That makes it possible later to swap in parakeet.cpp linked directly through FFI, which mobile will need because there is no sidecar there.

### 1.4 Speaker identity and naming
- LocalAI's speaker registry only lives in memory, so it is lost when the process restarts. **Veritas therefore keeps its own registry:**
  - Table `speakers(id, name, color, embedding BLOB, samples_count)`.
  - When the user clicks an unnamed "Speaker 2" chip and type "Anna", the app takes about 5–10 s of that speaker's audio from the segment timestamps, computes or fetches an embedding, and saves it.
  - At engine startup the app re-registers every known speaker with LocalAI (speaker-naming API). New sessions then show "Anna" automatically.
  - Renaming one speaker updates every segment in that meeting retroactively.
  - Speakers can be merged, for example when "Speaker 1" and "Speaker 3" turn out to be the same person.

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
