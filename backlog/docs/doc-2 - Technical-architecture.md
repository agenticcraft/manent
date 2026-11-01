---
id: doc-2
title: Technical architecture
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-11-01T10:37:00Z'
---
## 1. Technical foundation

### 1.1 App shell: Tauri 2 (Rust core with a TypeScript UI)
- **Why Tauri:** one codebase covers Linux, macOS and Windows, and Tauri 2 also targets iOS and Android, a candidate for the mobile companion app (decision-4; the platform is chosen in VRT-26.1). Installers are small: the Linux release binary is 8.8 MB and the deb/rpm 3.9 MB (VRT-29), against about 150 MB for Electron. Rust gives low-latency, safe audio threads.
- **Frontend:** React, TypeScript, Vite, Tailwind CSS and shadcn/ui.
- **Backend (Rust):** audio capture, encoding, storage, and managing the transcription engine. It talks to the UI through Tauri commands, which carry requests, and events, which carry streams such as audio levels and transcript segments.

### 1.2 Audio capture layer (Rust module `audio/`)
Each **source** is either a mic input or a system-output loopback, and each one is captured as its own track. **One crate, `cpal` (0.18 or newer), covers both kinds on every desktop OS** (validated on Linux in VRT-1):

| OS | Mic | System audio (loopback) |
|---|---|---|
| Linux | `cpal` PipeWire host (feature `pipewire`) | `cpal` PipeWire host: an input stream on an output (sink) device sets `stream.capture.sink`, which is loopback |
| Windows | `cpal` WASAPI | `cpal` WASAPI loopback: an input stream on an output device |
| macOS | `cpal` CoreAudio | `cpal` CoreAudio loopback on macOS 14.6 and newer (an input stream on an output device; cpal creates a process tap plus an aggregate device). Needs the system-audio capture permission; verified in VRT-12 |
| Android (companion, VRT-26) | AudioRecord | `AudioPlaybackCapture` (MediaProjection), which only works for apps that allow capture |
| iOS (companion, VRT-26) | AVAudioEngine | Not possible because of OS limits, so mic only |

Design:
- A `trait AudioSource` (format, start, stop) implemented by `CpalSource`; platform details (which devices are virtual) live in `cfg` modules. Device enumeration returns a `SourceInfo {id, name, kind: Mic|Loopback, is_default}`.
- A lock-free ring buffer (`rtrb`) on each source, so the realtime callback never blocks.
- Two consumers for each source:
  1. **Recorder:** writes each track to its own **FLAC** file at the native sample rate; on Stop the tracks are mixed into `mix.opus` for playback (§1.5).
  2. **Live transcription:** each track also feeds a caption worker, which resamples to **16 kHz mono f32** (`rubato`) and cuts 4–8 s chunks for the engine (§1.3).
- Live level meters (RMS and peak) are sent to the UI at about 20 Hz.
- **Track alignment:** the streams start about 20 ms apart (VRT-1) and the tracks are not shifted: that is small against the 1 s bleed window and inaudible in the mix. Over 5 minutes VRT-1 measured no clock drift between mic and system tracks. If it ever matters, the first callback's timestamp (`InputCallbackInfo`) can align them.
- **Key design advantage:** because the mic and system audio are separate tracks, the app already knows that speech on the mic track is the **local user ("Me")**. Diarization therefore only has to separate the remote voices on the system track, which makes the speaker labels much more accurate. **Bleed** (the mic hearing the speakers) is removed by text: a mic block goes when at least half its words match a system-track word (same text within 1 s), otherwise only the matching words go, so the system track wins on overlap while the user's own words, also when talking over someone, stay (`passes.rs`, VRT-17). Headphones avoid the problem entirely.

### 1.3 Transcription engine: libparakeet (parakeet.cpp) in a helper process
**One model for everything, Dutch and English (decided in VRT-32):** `parakeet-cpp-bundle-standard` (1.1 GB, Q8_0). It holds parakeet-tdt 0.6B v3 (ASR, 25 European languages, detects the language itself), Nemotron-3 diarization, WeSpeaker speaker embeddings, Silero VAD and a sound tagger. It serves live captions, the rolling and final passes, diarization and speaker embeddings. No streaming models are needed.

- `parakeet-cpp` is a C++/ggml port of NVIDIA NeMo Parakeet. It runs quantized GGUF models on CPU at upstream accuracy and returns **word-level timestamps**.
- **Engine decision (VRT-29, decision-1):** no LocalAI. The spikes used LocalAI, but its parakeet-cpp backend is a thin wrapper around `libparakeet.so`, whose C API (`parakeet_capi_*`, ABI v10, header in `spikes/engine/include/`) Veritas calls directly. Same results as through LocalAI (identical WER, diarization and embeddings), faster (RTF 0.064 vs 0.10), 0.6 s to load, 1.9 GB peak on a 17 min file.
- **Language:** auto only. The language option is ignored by parakeet 0.6B v3 (auto, `nl` and `en` give identical output), so the app has no language setting. Mixed Dutch/English meetings work because each chunk is detected on its own.
- **Measured** (test machine, CPU, libparakeet directly, VRT-29): RTF 0.064 for transcription, 0.026 for diarization. Offline WER on spontaneous English meetings 14–20% (AMI, VRT-31); Dutch 11% on read speech (FLEURS) and 8–17% disagreement with YouTube captions on Dutch interviews (VRT-32).
- **Rejected for live** (VRT-2, VRT-31, VRT-32): `realtime_eou_120m` streaming (English only; 28.9% WER on a real meeting, drops crosstalk, no live speaker segments) and `nemotron-3.5-asr-streaming-0.6b` (32% WER on mixed NL/EN, leaks language tags). LocalAI's realtime WebSocket with `server_vad` waits for pauses, so text lagged up to 34 s on fluent speech.
- **Licences** (NOTICE-parakeet-bundle-standard.txt): CC-BY-4.0 (ASR, WeSpeaker), OpenMDW-1.1 (diarization: permissive, keep the licence and notices when distributing), MIT (Silero VAD), Apache-2.0 per model card for the sound tagger (upstream licensing inconsistent; Veritas doesn't use it). The app shows the attributions; if the model is ever bundled, the NOTICE file ships with it.

How the app uses it:
- **Helper process `veritas-engine`** (a small Rust binary in the workspace, the only crate allowed `unsafe`): loads `libparakeet.so` with `libloading`, checks `parakeet_capi_abi_version() == 10`, loads the bundle's components (`parakeet_capi_load_component`: ASR with an empty name, `"vad"`, `"diar"`, `"voice"`) once, and serves requests from the app as JSON lines over stdin/stdout. The app supervises it (start, restart after a crash, stop). A crash or out-of-memory in the engine never stops a recording. No network port, so nothing to bind.
- **Delivery (downloaded at first run, nothing big in the installer):**
  - libparakeet from LocalAI's backend images on quay.io (`go-skynet/local-ai-backends`: `cpu-parakeet-cpp` for linux/amd64, `metal-darwin-arm64-parakeet-cpp` for macOS arm64), fetched over the OCI registry API at a pinned digest, every layer checked against its sha256 (`engine/download.rs`, pins in `engine/assets.rs`). 13.8 MB download; only `libparakeet.so` (3.9 MB) and `libgomp.so.1` are kept. The image's own glibc and libstdc++ must never be used (the helper then fails on a glibc mismatch); the OS provides them. Always the CPU variant. **No Windows, macOS Intel or Linux arm64 CPU build exists**: Windows builds parakeet.cpp from source (VRT-11/VRT-23).
  - The model `parakeet-bundle-standard.gguf` (1.1 GB) from Hugging Face, with progress, resume and checksum. Own mirror: VRT-33.
- **C API calls:**
  - Offline and rolling passes: `parakeet_capi_transcribe_path_json_vad_with(asr, vad, wav, 0, NULL)`: words with start/end/confidence. The 0.6B model has no VAD head, so the Silero `"vad"` component cuts long audio (≤ 30 s pieces). The engine writes a 16 kHz WAV for it. **Repair (VRT-17):** within a piece that switches language the model sometimes drops a whole sentence (Dutch-English-Dutch); the helper also runs the Silero VAD (`vad_path_json`) and transcribes speech regions of at least 1 s that got no words again on their own (`transcribe_pcm_batch_json`). On the NL/EN recording this took WER from 14.6% to 10.1%; it changed nothing on AMI. Shorter VAD pieces (`max_segment` 10-25 s) were not a fix: the result depends on where pieces happen to be cut and English meetings got worse.
  - Live chunks: the same call on a short-lived 16 kHz WAV in `<app data>/tmp/` (audio of 30 s or less is transcribed whole, without VAD): 0.5 s for a 6 s chunk, 0.9 s for 10.7 s (VRT-15). One request protocol (file paths) for everything; `transcribe_pcm_batch_json` would only save the WAV write.
  - Diarization and speaker embeddings: `parakeet_capi_diarize_profiles_pcm_json(diar, voice, NULL, pcm, n, 16000, 0, 0)`: segments plus `speaker_profiles` (256-dim WeSpeaker embeddings, `clean_duration`, encoder identity).
  - **Don't use** `parakeet_capi_transcribe_and_diarize*`: on the NL/EN sample it dropped every English turn and peaked at 9 GB on a 17 min file.
  - Calls on one context queue; `parakeet_capi_set_concurrency` lets live chunks run beside a rolling pass.
- **Three passes per meeting:**
  1. **Live captions (VRT-16, `live.rs`):** each track feeds its own worker thread with exactly what it writes (silence while muted, nothing while paused), so caption times are track milliseconds, the same timeline as the files and the mix. The worker downmixes, resamples to 16 kHz and cuts chunks as the audio arrives (no look-ahead):
     - from 4 s on, a chunk ends in the first pause: 160 ms of frames 13 dB below the chunk's median frame or below -55 dBFS (shorter dips occur inside words);
     - without a pause it ends by 8 s at the quietest 160 ms; the words ending within 300 ms of such a forced cut are dropped and their audio starts the next chunk, so a word cut in two is transcribed whole;
     - a chunk with sound that comes back empty (parakeet sometimes returns nothing for ~4 s pieces that start or end inside an utterance) is retried as part of a chunk at least 2 s longer, up to 12 s;
     - chunks below -60 dBFS are not sent (quiet remote speech can be around -50 dBFS).
     Measured on the real app with the FLEURS NL/EN sample played through the system audio: text a median 3-3.5 s and p90 5-7 s after it is spoken (a retried chunk up to ~10 s), live WER NL 11.2% and EN 11.5-16% on the system track (offline 11.2% / 11.5%). Captions are stored as `caption` segments with their track, shown in light grey per source ("Me" for the mic, "System audio"), and carry no speaker. (The VRT-32 simulation assumed a chunk is ready at its own end, which hid the 8 s look-ahead of a global quietest-frame cut.)
  2. **Rolling pass (VRT-17, `rolling.rs`):** about every 45 s, over the newest audio of every track (kept by the caption workers, last 3 min) plus 30 s of context: the system track is transcribed and diarized, the mic track transcribed as "Me", bleed removed. Words starting in `[from, to)` are kept, with `to` about 1 s before the newest audio and never inside a word; the result replaces the captions and earlier rolling segments of that stretch (event `transcript:pass`). Diarization slots are per call, so they get meeting labels ("1", "2", ...) by continuity with words already labelled in the context, then by voice (cosine distance <= 0.40 to an earlier label's embedding), otherwise a new label. Measured: text gets speakers a median 34-36 s after it is spoken (max ~50 s); 99% of words carry their person's label on AMI and NL/EN; an occasional extra label for a short spurious slot.
  3. **Final pass:** after Stop the meeting is `processing`; the whole tracks are decoded to 16 kHz and transcribed (system with diarization, mic as "Me"), bleed removed, speakers numbered in the order they first speak, and everything else replaced (pass `final`, event `transcript:final`), then `done`. This is the stored transcript. 5 min of audio took ~55 s (debug build), 74 s took ~18 s; 99-100% of words carry their person's label.
- **Jobs (VRT-19, `jobs.rs`):** the final pass, "Re-transcribe" and imports run as background jobs, one at a time (the engine never transcribes two whole meetings at once), with progress for the UI (`jobs:changed`; decoding by position, transcription estimated from the audio length). Jobs live in the app, so they survive navigation; a meeting left `processing` when the app quit is transcribed again at the next start.
- **Imports:** a file (wav, mp3, m4a/mp4/aac/mov, flac, ogg/vorbis, mkv/webm; decision-3) is decoded by symphonia into the meeting's mono FLAC track `import.flac` at its own rate (one track, kind `loopback`, so every voice is diarized; there is no "Me"), mixed into `mix.opus`, and transcribed like a recording's final pass, so known people are recognised. Import via the Library button (native dialog) or by dropping files on the window. Without the engine installed an import is refused.
- **Merging:** the app gives each word the speaker of the diarization segment it overlaps most (nearest segment when none overlaps). Diarization almost never assigns speech to the wrong person (0.1–0.3% on AMI, 6.6% in one meeting with a very quiet speaker); produced audio often yields extra tiny speaker slots (< 8 s of clean speech).
- An `Engine` trait in Rust (`transcribe_chunk`, `transcribe_file`, `transcribe_words`, `speaker_profiles`) hides the helper process. Phones don't transcribe: the companion app sends its recordings to a paired desktop (decision-4), so there is no mobile engine.
- **Not available without another engine:** LLM summaries (VRT-24) would need a separate local LLM runtime.

### 1.4 Speaker identity and naming
Validated in VRT-30 (LibriSpeech), VRT-31 (AMI, across sessions) and VRT-32 (Dutch interviews, across recordings and across Dutch/English): every enrolled person was named correctly and no unenrolled voice got a name.

- **Embeddings come from the engine, matching happens in the app.** `diarize_profiles_pcm_json` returns a 256-dim WeSpeaker embedding (and `clean_duration`) per speaker; they are identical to the ones LocalAI returned in the spikes (VRT-29). Veritas compares them with its own registry using cosine distance in Rust. It doesn't use libparakeet's in-memory speaker registry.
- **Registry:** tables `speakers(id, name, color)` and `voice_samples(id, speaker_id, embedding BLOB, encoder_id, clean_seconds, meeting_id, created_at)`. A person has several samples. Matching uses the nearest sample, and only samples from the same encoder (`encoder_id` = the `sha256:` identity in `speaker_profiles`).
- **Decision rule** (cosine distance to the nearest sample of each person): **auto-name** when the nearest person is ≤ 0.40 *and* at least 0.15 closer than any other enrolled person; **suggest** the nearest name (user confirms) when it is ≤ 0.60; otherwise leave the speaker unnamed ("Speaker N"). Measured: same person 0.06–0.39 (worst case: 6 s enrollment on degraded audio); different people ≥ 0.46 (two Dutch men in the same studio), usually ≥ 0.68.
- **Tiny slots:** diarization slots with < 5 s of clean speech are never auto-named and never used as samples; the user can merge them into a named speaker.
- **A meeting's voices (VRT-18):** table `meeting_voices(meeting_id, label, speaker_id, suggested_id, distance, embedding, encoder_id, clean_seconds, confirmed)` holds, per speaker label of a meeting, the cleanest embedding the passes found and who it is. After every rolling pass and the final pass the unconfirmed labels are matched with the decision rule (`speakers.rs`: auto-named, suggested or unnamed) and the segments get the person of their label. The final pass numbers speakers anew; names the user gave during the recording carry over to the label that covers at least half of the named stretches.
- **Naming a speaker:** when the user clicks an unnamed "Speaker 2" chip and types "Anna" (an existing person if the name matches, ignoring case, else a new one), that speaker's embedding from this meeting becomes a sample for Anna. Every confirmed or corrected name adds a sample, so matching adapts to real mics and rooms over time. Only speakers with at least 5 s of clean speech are used as samples.
- **Named transcript:** transcription segments get the name (or "Speaker N") of the diarization segment they overlap most. Consecutive segments of the same speaker form one block.
- **Me:** the mic track's speaker is the user, never diarized and not in the registry; the name shown for them is a setting (`speaker.me_name`, "Me" by default) and can be changed from its chip.
- Renaming a speaker updates every segment retroactively. Merging speakers (for example "Speaker 1" and "Speaker 3" are the same person) moves their samples to one person. Deleting a person deletes all their samples.

### 1.5 Storage
- **SQLite** via `sqlx` (bundled SQLite, WAL, foreign keys on) in the app data folder: `<app data>/veritas.db`, opened and migrated at app start (`src-tauri/migrations/`, implemented in VRT-6). Times are milliseconds: absolute since the Unix epoch, or relative to the meeting start.
  - `meetings(id, title, started_at, duration_ms, status ∈ recording|processing|done|failed, dir)`
  - `tracks(id, meeting_id → meetings, source_kind ∈ mic|loopback, source_id, device_name, path, seconds)`
  - `segments(id, meeting_id → meetings, track_id → tracks, pass ∈ caption|rolling|final, speaker_id → speakers, speaker_label, start_ms, end_ms, text, words_json)`: the three passes of §1.3; a rolling or final pass atomically replaces the segments of earlier passes in its time window.
  - `speakers(id, name, color)` and `voice_samples(id, speaker_id → speakers, embedding BLOB (LE f32), encoder_id, clean_seconds, meeting_id → meetings, created_at)` (§1.4).
  - `meeting_voices(meeting_id → meetings, label, speaker_id → speakers, suggested_id → speakers, distance, embedding, encoder_id, clean_seconds, confirmed)`: a meeting's speaker labels and who they are (§1.4, migration 0002).
  - `settings(key, value JSON)`: `recording.sources` (Settings > Sources), `speaker.me_name`, `transcription.rolling_interval_s` (30/45/60/90, default 45), `storage.keep` (`all` or `mix`), `storage.meetings_dir` (null = `<app data>/meetings`) (VRT-27).
- Deleting a meeting cascades to its tracks and segments; deleting a person deletes their voice samples (segments stay, unnamed).
- **Search:** `segments_fts` (FTS5, `unicode61 remove_diacritics 2`, kept in sync by triggers). Each word is prefix-matched and all words must match; diacritics and case are ignored, so "ideeen" finds "ideeën". Matches come back with a snippet whose matched words are wrapped in U+0001/U+0002 (transcripts can contain brackets); the Library shows them under "In transcripts" and opens the meeting at the match (VRT-20).
- **Export (VRT-21, `export.rs`):** Markdown, plain text, SRT (cues of at most 7 s, split between words) and JSON (blocks with their words and times) from the stored segments, speakers named as in the app; written where the save dialog says. Snapshot tests in `src-tauri/tests/snapshots` (`UPDATE_SNAPSHOTS=1` to accept a change).
- **Waveform:** `meeting_waveform` computes 600 loudness bars (RMS per bin, loudest track, scaled to the 98th percentile) from the FLAC tracks and caches them as `waveform.json` in the meeting folder once the meeting is done.
- The engine lives in `<app data>/engine/` (libparakeet, libgomp, `engine.json` naming the image manifest and the sha256 of each kept file, so Settings > Check model finds damage) and the model in `<app data>/models/` (the bundle and its NOTICE). Interrupted downloads stay as `*.part` and resume (VRT-14).
- **Moving the meetings folder (VRT-27):** Settings > Storage moves every meeting folder (rename, or copy and delete across file systems), then points `meetings.dir` and `tracks.path` to the new place; refused while recording or transcribing. The root is shared by every part of the app (`AppPaths`), so the media server and jobs use it at once.
- **Keeping only the playback file (VRT-27):** with `storage.keep = mix`, a meeting's FLAC tracks are deleted after its final pass (the waveform is cached first; never without `mix.opus`). Re-transcribe then explains that the tracks are gone. Measured sizes: FLAC tracks of a recording ~400 MB per hour, `mix.opus` ~13 MB per hour.
- Each meeting has a folder `<app data>/meetings/<id>/` holding `mic.flac` and `system.flac` (one file per source, §1.2), or `import.flac` for an imported file, and `mix.opus`.
- **Playback mixdown (VRT-10):** on Stop the tracks are mixed into `mix.opus`: mono, 48 kHz, Opus at 32 kbps (~14 MB per hour), streamed (never the whole meeting in memory), tracks at other sample rates resampled. Ogg pages end every second so players can seek. libopus is built statically from source by the `opus` crate (needs cmake, no system library).
- **Playback transport:** the webview plays `mix.opus` from a local media server inside the app (`media_server.rs`): bound to 127.0.0.1 on a random port, a random 128-bit token in every URL, only `meetings/<id>/mix.opus` inside the meetings folder, HTTP Range support, one thread per request. Reasons: WebKitGTK can't play media from Tauri's custom protocols, and from `blob:` URLs it can't seek. The player uses `preload="auto"`; with `metadata` WebKitGTK never learns an Ogg file's duration.

### 1.6 Project layout
```
veritas/
  src-tauri/                 # Cargo workspace: the app and the engine helper
    src/
      main.rs, lib.rs        # entry point; Tauri setup, state, command registration
      commands.rs            # Tauri commands (kept in sync with src/lib/ipc.ts)
      audio/                 # capture (cpal), ring buffers, recorder, FLAC, levels, resampler,
                             # mixdown, import decoding, waveform; linux/windows/macos.rs
      live.rs                # live captions: per-track chunker and caption workers (VRT-16)
      rolling.rs, passes.rs  # rolling and final passes; labels, bleed removal (VRT-17)
      speakers.rs            # registry matching, doc-2 §1.4 decision rule (VRT-18)
      jobs.rs                # background job queue: final pass, re-transcribe, imports (VRT-19)
      engine/                # EngineManager, helper client, downloads, pins, transcript types
      db.rs                  # sqlx queries; migrations/ next to src/
      media_server.rs        # local playback server for mix.opus
      paths.rs               # data folders (VERITAS_DATA_DIR override)
    engine-helper/           # veritas-engine: libparakeet FFI, JSON lines on stdin/stdout
    tests/fixtures/          # FLEURS speech and format fixtures (see its README)
  src/                       # React UI
    routes/                  # Library, Live, Meeting, Speakers, Settings
    components/              # TranscriptBlocks, SpeakerChip, AudioPlayer, JobsPanel, ...
    components/ui/           # shadcn/ui components
    lib/                     # ipc.ts (typed commands/events), transcript.ts, format.ts, theme.ts
    index.css                # design tokens (doc-3)
  scripts/                   # smoke checks of the real app via tauri-driver (AGENTS.md)
  spikes/                    # Phase 0 spikes: audio, LocalAI, engine prototype and header
```
