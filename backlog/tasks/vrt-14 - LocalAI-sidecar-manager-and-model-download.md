---
id: VRT-14
title: Engine helper process and engine/model download
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T09:37:00Z'
labels:
  - transcription
  - backend
milestone: m-2
dependencies:
  - VRT-3
  - VRT-29
  - VRT-32
documentation:
  - doc-2
priority: high
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run the transcription engine as the veritas-engine helper process (decision-1, VRT-29): a small Rust crate that loads libparakeet.so with libloading, checks ABI v10, loads the bundle components (ASR, vad, diar, voice) once and serves JSON-line requests over stdin/stdout. The app starts it on demand, detects crashes and restarts it; a crash never affects a recording. At first run the app downloads, with progress, resume and checksum verification: libparakeet from LocalAI's backend image on quay.io at a pinned digest (CPU variant, only lib/libparakeet.so kept, plus libgomp if the OS lacks it; see spikes/engine/fetch_backend.py), and the model parakeet-bundle-standard.gguf (1.1 GB) from Hugging Face. Show the model attributions (NOTICE). Prototype and measurements: spikes/engine/proto. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Engine and model download in-app with progress, sha256/digest verification, and resume after an interrupted download or app restart
- [x] #2 Helper starts on demand, reports ABI and loaded components, and stops on app exit
- [x] #3 A helper crash is detected and the helper restarted; a running recording is not affected
- [x] #4 Clear error when the platform has no prebuilt libparakeet or the ABI version doesn't match
- [x] #5 Model attributions/licences shown in the app
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Workspace: src-tauri becomes a Cargo workspace with member engine-helper/ (crate veritas-engine, the only crate allowed unsafe). Helper: args --lib --model; loads libparakeet with libloading, checks ABI 10, loads components asr/vad/diar/voice, set_concurrency(asr, 2, cores/2); prints a ready line; then JSON-line requests on stdin (id, op transcribe|diarize, wav path), one thread per request, responses with the same id on stdout; exits on stdin EOF.
2. App engine/assets.rs: pinned downloads per platform (linux-x86_64 image manifest sha256:8497e3e3..., macOS arm64 sha256:25bbd382...; model parakeet-bundle-standard.gguf rev 15a3c9ed, 1100752928 bytes, sha256 85304e3b...; NOTICE sha256 73a1f196...). Windows/other: unsupported (clear error).
3. App engine/download.rs (ureq, decision-2): resumable file download (.part + Range, hash existing part then continue, size and sha256 check, cancel flag, progress callback); OCI fetch: manifest by digest (hash-checked), layer blobs hash-checked, keep lib/libparakeet.* and libgomp.so.1. Unit tests with a local tiny_http server (resume, wrong hash, cancel).
4. App engine/helper.rs: spawn helper (next to the app binary) with LD_LIBRARY_PATH/DYLD_LIBRARY_PATH = engine dir; reader thread routes responses to pending requests (tokio oneshot); on exit fail pending requests, emit engine:status crashed, restart automatically (backoff, give up after 3 quick crashes). Stop on app exit (stdin closed + kill).
5. Commands engine_status, engine_download, engine_cancel_download, engine_start, engine_notice; event engine:status. Typed in ipc.ts.
6. UI Settings > Transcription: engine/model status, download with progress and cancel, start/test, model licences (NOTICE).
7. Verify: unit tests; download the real engine into a temp data dir; resume test for the model by seeding .part from the local copy; helper start; kill -9 the helper during a recording -> restarted, recording intact; smoke script smoke-engine.py; screenshots light/dark.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-30):
- src-tauri is now a Cargo workspace; engine-helper/ = veritas-engine (only crate with unsafe). JSON lines over stdin/stdout, one thread per request, set_concurrency(asr, 2 backends). Ready line reports ABI/components/load time.
- engine/assets.rs pins: quay manifest sha256:8497e3e3 (linux-x86_64; only libparakeet.so + libgomp.so.1 kept, the image's glibc must NOT be used: it breaks the helper), sha256:25bbd382 (macOS arm64, untested), model + NOTICE at HF revision 15a3c9ed with sha256.
- engine/download.rs (ureq, decision-2): .part + Range resume, hash of the existing part, size+sha256 check, cancel keeps the part; OCI manifest hash-checked by digest. 7 unit tests against a local tiny_http server (resume, dropped connection, server without Range, wrong checksum, cancel, image install keeps only pinned files).
- engine/helper.rs + mod.rs: EngineManager (status/events, download thread, start on demand, crash -> restart after 1 s, gives up after >3 crashes/min, shutdown on RunEvent::Exit; helper also exits on stdin EOF).
- UI: Settings > Transcription (download with size, progress, cancel; start; state with load time and restart count; errors; model licences).
Verification: scripts/smoke-engine.py passed twice from an empty data folder (engine 14 MB from quay + last 40 MB of the model from HF with a cancel mid-model and a Range resume; sha256 matches; ABI 10, 4 components, load 0.66-0.7 s; kill -9 of the helper during a recording -> restarted in ~1 s, restarts=1 shown, recording continued and both tracks complete; helper gone after app exit). ABI mismatch shown in the app with a fake ABI-9 library; missing library/model give clear errors. The 'no prebuilt for this platform' branch can't run on Linux (compile-time cfg); it shows a message instead of the download button.
- One earlier smoke run found the UI on the Library during a recording (recording itself fine); not reproduced in 3 later runs incl. an isolated repro; the smoke now checks and screenshots it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Engine delivery and supervision: the veritas-engine helper (libparakeet over FFI, JSON lines) is built with the app; the app downloads the pinned engine (14 MB, quay.io) and model (1.1 GB, Hugging Face) with progress, cancel, resume and sha256 checks, starts the helper on demand and restarts it after a crash without affecting recordings. Settings > Transcription shows download, state, errors and model licences. Verified with unit tests and smoke-engine.py on the real app (download/cancel/resume, ABI 10, kill -9 during a recording). Risk: macOS image pin untested; Windows has no engine yet (VRT-23).
<!-- SECTION:FINAL_SUMMARY:END -->
