---
id: VRT-34
title: Evaluate a smaller quantised model (split q4_k/q5_k ASR + diar/voice/vad)
status: To Do
assignee: []
created_date: '2026-10-30T09:03:00Z'
labels:
  - spike
  - transcription
milestone: m-4
dependencies: []
priority: low
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The one model today is parakeet-bundle-standard.gguf (1.1 GB, Q8_0, all components). huggingface.co/mudler/parakeet-cpp-gguf also publishes the ASR model alone as tdt-0.6b-v3 q4_k (675 MB), q5_k (742 MB), q6_k (813 MB) and q8_0 (941 MB); the diarization, WeSpeaker and Silero components would then come as separate files. Measure download size, RAM, speed and the accuracy cost (FLEURS NL/EN, AMI ES2004a, Dutch interviews; same scripts as VRT-29: spikes/engine/proto, score_ami.py, to_verbose.py) and check that speaker embeddings stay identical (encoder_id). Only switch if the saving is worth the accuracy loss and the extra files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 WER, RTF and RAM measured for q4_k/q5_k against the Q8_0 bundle on the VRT-29 samples
- [ ] #2 Decision: keep the bundle or switch to split files, recorded in doc-2 §1.3
<!-- AC:END -->
