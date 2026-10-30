---
id: decision-1
title: >-
  Engine: libparakeet in a helper process, no LocalAI; engine and model
  downloaded at first run
date: '2026-10-30T09:00:00Z'
status: accepted
---
## Context

Veritas needs only three things from the engine: transcription with word timestamps, diarization with speaker embeddings, and VAD. All of them come from one model file, parakeet-cpp-bundle-standard (VRT-32). LocalAI, which was used in the spikes, is a 187 MB binary that pulls in LLMs, image generation, a web UI and galleries, and picks GPU backends of several GB by itself. The app shell itself is small: the release binary is 8.8 MB, and the deb/rpm packages are 3.9 MB.

LocalAI's parakeet-cpp backend is a thin wrapper around libparakeet.so (parakeet.cpp). That library has a versioned C API, `parakeet_capi_*` at ABI v10. VRT-29 called it directly from a Rust prototype (spikes/engine/proto, using libloading) and compared the results with the LocalAI runs on the same files:

| | LocalAI (VRT-31/32) | libparakeet directly |
|---|---|---|
| WER on FLEURS NL/EN (74 s) | 11.3% (NL 11.2, EN 11.5) | 11.3% (NL 11.2, EN 11.5) |
| WER on AMI ES2004a (17.5 min) | 16.3% | 16.3% |
| Diarization on ES2004a (right / wrong / missed) | 77.6 / 0.3 / 22.1% | 77.6 / 0.3 / 22.1% |
| Speaker embeddings | WeSpeaker, 256-dim | identical (cosine distance 0.000; same encoder identity hash) |
| RTF, transcription / diarization | 0.101 / 0.031 | 0.064 / 0.026 |
| Live 6 s chunk | (HTTP round trip) | median 0.40 s, max 0.50 s |
| Startup | LocalAI process plus backend spawn | dlopen 2 ms, model components loaded in 0.6 s |
| Peak memory (17.5 min file) | | 1.9 GB |
| Download | 187 MB binary plus backend | 13.8 MB compressed image; libparakeet.so is 3.9 MB, and the rest is glibc/libstdc++/libgomp, which the OS already has |

Prebuilt libparakeet is published in LocalAI's backend images at quay.io/go-skynet/local-ai-backends:
- linux/amd64 CPU: latest-cpu-parakeet-cpp
- macOS arm64 Metal: latest-metal-darwin-arm64-parakeet-cpp
- GPU variants for Linux

There is **no Windows build**, and no macOS Intel or Linux arm64 CPU build.

The test also found one trap. `parakeet_capi_transcribe_and_diarize_json` (speaker-attributed ASR) dropped every English turn in the NL/EN sample and peaked at 9 GB. The 0.6B model also has no VAD head, so long files need the bundle's separate Silero "vad" component (`transcribe_path_json_vad_with`).

## Decision

- **No LocalAI.** Veritas drives libparakeet directly through its C API.
- **Helper process:** the engine runs in a small Rust helper (`veritas-engine`), which loads libparakeet with libloading and talks to the app over stdin/stdout. If the engine crashes or runs out of memory, a recording keeps going. `unsafe` FFI code is allowed only in the helper crate; the app keeps `unsafe_code = forbid`.
- **API calls the engine uses:**
  - ASR: `transcribe_path_json_vad_with` (offline and rolling passes, with the Silero "vad" component) and `transcribe_pcm_batch_json` (live chunks).
  - Diarization and embeddings: `diarize_profiles_pcm_json` ("diar" and "voice" components).
  - Merging: the app merges words and speakers itself (doc-2 §1.4).
  - Not used: `transcribe_and_diarize*`.
- **Downloads at first run (nothing big in the installer):**
  - libparakeet from the LocalAI backend image for the OS, through the OCI registry API, with every layer checked against its sha256 digest. Only `lib/libparakeet.so` is kept, plus libgomp where the OS lacks it.
  - The model (parakeet-bundle-standard.gguf, 1.1 GB) from Hugging Face, with progress, resume and a checksum.
  - An own mirror is VRT-33.
- **Version pinning:** the app checks `parakeet_capi_abi_version() == 10` and pins image digests rather than `latest` tags.
- **Windows:** libparakeet is built from source (parakeet.cpp with CMake) in VRT-11/VRT-23 until a prebuilt exists.

## Consequences

- The installer stays at the app size (about 4–9 MB). The first run downloads about 14 MB of engine and 1.1 GB of model.
- The engine abstraction (doc-2 §1.3) now covers the libparakeet helper. Mobile can later link the same C API in-process.
- The behavior of LocalAI's HTTP layer that Veritas relied on moves into Veritas. That means a job queue, word/speaker merging, and choosing the CPU backend explicitly (no GPU auto-pull).
- Not available without another engine: LLM summaries (VRT-24) and the OpenAI-compatible API. Speaker naming was already planned in-app, with embeddings in SQLite.
- Upstream risk: parakeet.cpp's API changes fast. The ABI check plus pinned digests contain this, and an ABI bump becomes a deliberate update.
- Windows needs its own build pipeline for libparakeet.

