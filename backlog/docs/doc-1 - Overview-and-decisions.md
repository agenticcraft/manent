---
id: doc-1
title: Overview and decisions
type: readme
created_date: 2026-10-29 12:18
updated_date: '2026-10-29T18:44:00Z'
---
## Context
Veritas is a privacy-first desktop app that records meetings from **both your microphone and your system audio output**, then transcribes them **locally on the CPU**. The transcript is live, shows who is speaking, lets the user put names on voices, and also works on recordings imported later. It runs first on the test machine (Fedora 44, PipeWire), then on macOS and Windows, and maybe on mobile.

## Decisions
- **Languages:** **Dutch and English**, including meetings that switch between them. The language is detected automatically per chunk; there is no language setting (the model ignores it). **One model** serves everything: `parakeet-cpp-bundle-standard` (1.1 GB, parakeet 0.6B v3 with diarization, speaker embeddings and VAD), for live captions (app-side 4–8 s chunks), the rolling and final passes, and imports (decided in VRT-32). The test machine (Ryzen AI 9 HX 370) runs it at about 10x real time on CPU.
- **Stack:** Tauri 2 with Rust and React.
- **Engine:** LocalAI runs as a sidecar that the app bundles and starts itself.
- **Extras:** a global hotkey and a tray icon are in scope. AI summaries and action items are **on the backlog**, not built now.

## Roadmap
The work is tracked as Backlog.md tasks grouped into milestones. The detail for each step lives in its task, so it isn't repeated here.
- **Phase 0 - Spikes:** remove the main risks: audio capture on Linux, LocalAI parakeet on CPU, speaker naming, English on real meetings (VRT-31), then Dutch and mixed Dutch/English (VRT-32).
- **Phase 1 - Recorder (stage 1):** a cross-platform app that records the mic and system audio as separate tracks, with configurable sources.
- **Phase 2 - Transcription (stage 2):** live transcription with diarization, named speakers, and transcription of recordings afterwards.
- **Phase 3 - Polish:** tray icon, global hotkey, packaging and auto-update.
- **Backlog:** AI summaries, auto-detecting meetings, and mobile.

## Related docs
- doc-2 Technical architecture
- doc-3 UI design
