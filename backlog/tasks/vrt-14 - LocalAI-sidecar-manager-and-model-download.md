---
id: VRT-14
title: Engine helper process and engine/model download
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T09:02:00Z'
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
- [ ] #1 Engine and model download in-app with progress, sha256/digest verification, and resume after an interrupted download or app restart
- [ ] #2 Helper starts on demand, reports ABI and loaded components, and stops on app exit
- [ ] #3 A helper crash is detected and the helper restarted; a running recording is not affected
- [ ] #4 Clear error when the platform has no prebuilt libparakeet or the ABI version doesn't match
- [ ] #5 Model attributions/licences shown in the app
<!-- AC:END -->
