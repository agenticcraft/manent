# Veritas: agent instructions

Veritas is a local-first desktop app that records meetings (the mic and system audio as separate tracks) and transcribes them live with speaker identification, using parakeet.cpp (`libparakeet`) on the CPU.

## Where things live
- **Product context, decisions and roadmap:** Backlog doc-1 (`backlog doc view doc-1`).
- **Technical architecture:** doc-2. **UI design:** doc-3.
- **Work items:** Backlog tasks (`VRT-*`), grouped by milestone (Phase 0-3, Backlog).
- Don't duplicate information. A fact lives in one place (a task, a doc, or this file), and other places link to it.

## How we build this project
- **Stack changes:** stick to the stack in doc-1. Adding a framework or major dependency needs a Backlog decision (`backlog decision create`).
- **Local first and private:** audio, transcripts and voice embeddings never leave the machine. No cloud APIs or telemetry. The engine and model downloads are the only network traffic; the engine helper talks to the app over stdin/stdout, and the media server binds to `127.0.0.1` only. Voice embeddings are biometric data, so users must always be able to delete them.
- **Spike before you build:** risky unknowns (OS audio APIs, engine APIs, model behavior) get a Phase 0-style spike first. Record the findings in that task's implementation notes before you build on them.
- **Linux first, cross-platform by design:** develop and verify on Linux (Fedora, PipeWire). Keep platform code behind traits and `cfg` modules (`audio/linux.rs`, `windows.rs`, `macos.rs`), never mixed into shared logic.
- **Audio threads:** realtime callbacks never block, allocate or lock. They only push into lock-free ring buffers (`rtrb`). All other work (encoding, resampling, ASR) runs on consumer threads.
- **One track per source:** mic and system audio are always captured and stored separately. The mic track is "Me". Don't mix down before transcription.
- **Engine abstraction:** all transcription goes through the Rust `Engine` trait. The libparakeet FFI lives only in the `veritas-engine` helper process (the only crate that may use `unsafe`), so an engine crash never stops a recording. Mobile can link the same C API in-process later.
- **Verify the APIs you use:** parakeet.cpp changes quickly. Check functions and options against the pinned `parakeet_capi.h` (ABI v10, `spikes/engine/include/`), not from memory.
- **IPC:** the UI talks to Rust only through typed wrappers in `src/lib/ipc.ts`. Commands carry requests, and events carry streams (levels, segments).
- **UI:** follow doc-3. It should be calm and minimal, with the transcript as the hero, neutral colors plus one accent, and light and dark modes.

## Testing and quality
- Rust: unit tests for the resampler, the segment merge and bleed logic, and the DB queries. An integration test sends a fixture WAV through the engine.
- UI: smoke-test the real app, not just a browser: build with `npm run tauri build -- --debug --no-bundle`, start `tauri-driver` (`cargo install tauri-driver --locked`; needs `WebKitWebDriver`), then run `python3 scripts/ui-smoke.py src-tauri/target/debug/veritas <screenshot dir>` (plus `scripts/smoke-library.py` for the Library and `scripts/smoke-live.py <app> <data dir> <shots> [tone.wav]` for recording and `scripts/smoke-settings.py` for Settings > Sources and `scripts/smoke-playback.py <app> <data dir> <shots> [tone.wav]` for the mixdown and player; `scripts/smoke-engine.py <app> <data dir> <shots> --seed-model <local gguf>` for the engine download, start and crash restart (downloads ~56 MB); `scripts/smoke-captions.py <app> <data dir with the engine> <shots> <speech.wav>` for live captions (latency and per-language WER; play a loudness-normalised copy of spikes/localai/samples/meeting-nl-en.wav with its .truth.json next to it); `scripts/smoke-speakers.py <app> <data dir with the engine> <shots> <speech.wav> [--rename]` for speaker labels, rolling and final pass (truth with voices next to the WAV, e.g. the AMI ES2004a live window or the NL/EN sample); `scripts/smoke-naming.py <app> <data dir with the engine> <shots> <first.wav> <second.wav> [--merge-delete]` for naming, recognition after an app restart, and merge/delete on the Speakers screen (e.g. AMI ES2004a then ES2004b, same people); `scripts/smoke-import.py <app> <data dir with the engine> <shots> <file>...` for imports (any format, truth next to the file) and Re-transcribe; these need audio hardware, and smoke-settings briefly switches the OS default mic and restores it). Always start `tauri-driver` with `VERITAS_DATA_DIR=<empty temp folder>` so tests never touch the real app data (database and recordings). Extend the script when a ticket adds UI, and look at the screenshots (light and dark).
- Frontend checks: `npm run check` (tsc, ESLint, Prettier). Rust checks: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` in `src-tauri/`.
- Before you mark a task done, `cargo fmt`, `cargo clippy`, the lint, and the tests must all pass, and every acceptance criterion must be checked.

## Git
- Never add AI attribution to commits or PRs: no `Co-Authored-By` naming an AI, no "Generated with" lines. Commits are authored by the user alone.

<!-- BACKLOG.MD GUIDELINES START -->
<CRITICAL_INSTRUCTION>

## Backlog.md Workflow

This project uses Backlog.md for task and project management.

**For every user request in this project, run `backlog instructions overview` before answering or taking action.**

Use the overview to decide whether to search, read, create, or update Backlog tasks.

Use the detailed guides when needed:
- `backlog instructions task-creation` for creating or splitting tasks
- `backlog instructions task-execution` for planning and implementation workflow
- `backlog instructions task-finalization` for completion and handoff

Use `backlog <command> --help` before running unfamiliar commands. Help shows options, fields, and examples.

Do not edit Backlog task, draft, document, decision, or milestone markdown files directly. Use the `backlog` CLI so metadata, relationships, and history stay consistent.

</CRITICAL_INSTRUCTION>
<!-- BACKLOG.MD GUIDELINES END -->
