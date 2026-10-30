---
id: VRT-29
title: >-
  Investigate installer size: lean app, models and LocalAI downloaded after
  install
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29T15:46:00Z'
updated_date: '2026-10-30T09:04:00Z'
labels:
  - spike
  - release
  - transcription
milestone: m-2
dependencies:
  - VRT-2
  - VRT-32
documentation:
  - doc-2
priority: high
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Concern: embedding everything (LocalAI, backends, models) would make Veritas a huge download. Goal: a lean installer; models (and possibly the engine) are downloaded from inside the app after install, on demand.

Investigate and decide:
1. Baseline: how big is a Tauri 2 app shell + our Rust core per OS (installer and installed size)?
2. LocalAI footprint: the full local-ai binary is 187 MB (v4.12.1, linux-amd64) and includes far more than we use (LLMs, image gen, agents, web UI, galleries). Options:
   a. Ship the full binary as a sidecar (simplest, biggest).
   b. Download LocalAI on first run instead of bundling it.
   c. Use a slimmer LocalAI build (check for build tags/variants that include only the gRPC backend runner + audio endpoints).
   d. Skip LocalAI and run the parakeet-cpp backend directly (Veritas only needs /v1/audio/transcriptions and /v1/audio/diarization, no realtime WebSocket, no voice registry): the CPU backend (cpu-parakeet-cpp) is only 29 MB and is a gRPC server LocalAI talks to; or link parakeet.cpp (C++/ggml) through FFI into our Rust core (the Engine trait in doc-2 §1.3 already allows this; mobile needs it anyway).
3. Models: never bundled. Downloaded in-app with progress, checksum verification and resume. VRT-32 reduced the model set to ONE file for Dutch and English: parakeet-cpp-bundle-standard (1.1 GB Q8_0; live chunks, offline passes, diarization, embeddings). Streaming models (realtime_eou_120m 254 MB, nemotron-3.5 938 MB) and the local-store backend are not needed. Evaluate a smaller quantisation of the bundle if one is published, and measure its accuracy cost.
4. Backends: LocalAI auto-detects GPU and pulls multi-GB builds (ROCm seen in VRT-2); the app must pick the CPU variant explicitly.
5. Downloads: where they come from (Hugging Face, quay.io OCI), mirrors, and behaviour when offline or on slow links (VRT-2 saw ~20 KB/s single-connection GitHub downloads; parallel range requests fixed it, and HF downloads stalled once).

Output: a decision (backlog decision) on how the engine is packaged and delivered, with measured sizes, and updates to doc-2 and VRT-14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Installed and installer size measured for a bare Tauri 2 app on Linux (estimates for Windows/macOS)
- [x] #2 LocalAI packaging options (full sidecar, download on first run, slim build, direct parakeet-cpp backend or FFI) compared on size, effort and maintenance
- [x] #3 Model download strategy defined: in-app, on demand, with progress, checksum and resume; quantized variants evaluated
- [x] #4 Decision recorded with backlog decision create, and doc-2 + VRT-14 updated accordingly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Measure the app itself: release build + bundle (rpm/deb/AppImage) size on Linux; estimate Windows/macOS.
2. Prebuilt libparakeet per OS: list LocalAI backend images (quay.io/go-skynet/local-ai-backends: cpu/metal/... parakeet-cpp), check which OS/arch exist, fetch an image's layers over the registry HTTP API (no docker), extract lib/, record sizes + how to verify (digest).
3. Helper prototype (Rust, libloading): load libparakeet + the model, transcribe a fixture to JSON with word timestamps and run diarize_profiles; compare output and speed with the LocalAI results from VRT-2/31/32 on the same files; measure helper size and startup.
4. Record a backlog decision (engine packaging/delivery) with measured numbers; update doc-1 (engine decision), doc-2 §1.3/§1.6, AGENTS.md (engine abstraction), VRT-14/VRT-15 (helper instead of LocalAI sidecar), VRT-23 (packaging).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
User decisions (2026-10-30 grilling): engine = lean parakeet only, no LocalAI (Veritas uses 2 of its endpoints; libparakeet.so is 3.9 MB and its C API (96 parakeet_capi_* functions, ABI-versioned) covers transcription with word timestamps, VAD, diarization with speaker profiles/embeddings, speaker embeddings, streaming). Run it in a small helper process (crash isolation: a recording must survive an engine crash). libparakeet comes from LocalAI's prebuilt backend images per OS, downloaded at first run (risk: Windows builds may not exist - verify). Model from Hugging Face (official) with checksum + resume; own mirror = VRT-33 (backlog). Audience: only the developer for now (no signing/auto-update pressure). LLM summaries 'maybe someday' -> would get a separate engine later.

Findings (2026-10-30, spikes/engine):
- App alone: release binary 8.8 MB, deb/rpm 3.9 MB (Linux). Windows/macOS estimate similar (Tauri uses the system webview): ~5-10 MB installer.
- LocalAI options compared: full sidecar 187 MB + backend (biggest, GPU auto-pull, HTTP layer we don't need); download-on-first-run (same footprint, later); slim build (no published variant); direct backend image = libparakeet.so 3.9 MB (13.8 MB compressed image incl. glibc/libstdc++/libgomp, which the OS already has). Chosen: libparakeet via its C API in a helper process.
- Prebuilt libparakeet (quay.io/go-skynet/local-ai-backends): linux/amd64 CPU, macOS arm64 Metal, Linux GPU variants. None for Windows, macOS Intel, Linux arm64 CPU -> Windows built from source (VRT-23).
- Prototype spikes/engine/proto (Rust, libloading): dlopen 2 ms, components asr+vad+diar+voice loaded in 0.6 s. Identical to LocalAI: FLEURS NL/EN WER 11.3% (NL 11.2, EN 11.5); AMI ES2004a WER 16.3%, diarization 77.6/0.3/22.1%; embeddings cosine distance 0.000 (same encoder identity). Faster: RTF 0.064 transcription (LocalAI 0.101), 0.026 diarization. Live 6 s PCM chunk median 0.40 s, max 0.50 s. Peak memory 1.9 GB on 17.5 min.
- Traps: the 0.6B model has no VAD head -> transcribe_path_json_vad_with with the bundle's Silero 'vad' component. transcribe_and_diarize_json dropped every English turn on the NL/EN sample and peaked at 9 GB -> not used; the app merges words and speakers itself (spikes/engine/to_verbose.py shows the merge).
- Models: one file (parakeet-bundle-standard.gguf, 1.1 GB Q8_0) from Hugging Face, downloaded in-app with progress, checksum, resume (VRT-14). No smaller bundle is published, but the ASR model alone exists as tdt-0.6b-v3 q4_k (675 MB) to q8_0 (941 MB), with the other components as separate files. Not measured here: it doesn't change the engine decision, so it is VRT-34 (backlog).
- Decision: decision-1. Updated doc-1, doc-2 §1.3/§1.4/§1.6, AGENTS.md, VRT-14, VRT-15, VRT-18, VRT-23, VRT-24.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Engine decided (decision-1): no LocalAI. libparakeet (parakeet.cpp C API, ABI v10) runs in a small Rust helper process; libparakeet (~14 MB download) and the model (1.1 GB) are downloaded at first run, so the installer is just the app (~4-9 MB). A prototype proved identical WER, diarization and speaker embeddings to LocalAI, faster (RTF 0.064 vs 0.10), 0.6 s startup, 0.4 s per live chunk, 1.9 GB peak. Windows has no prebuilt libparakeet and needs a source build (VRT-23). Docs and VRT-14/15/18/23/24 updated.
<!-- SECTION:FINAL_SUMMARY:END -->
