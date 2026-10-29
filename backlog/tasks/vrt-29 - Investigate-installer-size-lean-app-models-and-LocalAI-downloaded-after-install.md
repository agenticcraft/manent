---
id: VRT-29
title: >-
  Investigate installer size: lean app, models and LocalAI downloaded after
  install
status: To Do
assignee: []
created_date: '2026-10-29T15:46:00Z'
updated_date: '2026-10-29T17:05:00Z'
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
   d. Skip LocalAI and run the parakeet-cpp backend directly: the CPU backend (cpu-parakeet-cpp) is only 29 MB and is a gRPC server LocalAI talks to; or link parakeet.cpp (C++/ggml) through FFI into our Rust core (the Engine trait in doc-2 §1.3 already allows this; mobile needs it anyway).
3. Models: never bundled. Downloaded in-app with progress, checksum verification and resume. Known sizes: parakeet-cpp-bundle-standard 1.1 GB (offline), realtime_eou_120m 254 MB f16 / 123 MB q4_k (live, English), nemotron-3.5 streaming 685-938 MB (live, Dutch candidate, VRT-32). Veritas supports Dutch and English, so the download size covers the model set for both languages; consider downloading only the models for the languages a user enables. Consider quantized variants (q8_0/q5_k) to cut download size and measure the accuracy cost.
4. Backends: LocalAI auto-detects GPU and pulls multi-GB builds (ROCm seen in VRT-2); the app must pick the CPU variant explicitly.
5. Downloads: where they come from (Hugging Face, quay.io OCI), mirrors, and behaviour when offline or on slow links (VRT-2 saw ~20 KB/s single-connection GitHub downloads; parallel range requests fixed it, and HF downloads stalled once).

Output: a decision (backlog decision) on how the engine is packaged and delivered, with measured sizes, and updates to doc-2 and VRT-14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Installed and installer size measured for a bare Tauri 2 app on Linux (estimates for Windows/macOS)
- [ ] #2 LocalAI packaging options (full sidecar, download on first run, slim build, direct parakeet-cpp backend or FFI) compared on size, effort and maintenance
- [ ] #3 Model download strategy defined: in-app, on demand, with progress, checksum and resume; quantized variants evaluated
- [ ] #4 Decision recorded with backlog decision create, and doc-2 + VRT-14 updated accordingly
<!-- AC:END -->
