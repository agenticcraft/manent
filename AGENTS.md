# Veritas: agent instructions

Veritas is a local-first desktop app that records meetings (the mic and system audio as separate tracks) and transcribes them live with speaker identification, using LocalAI + parakeet on the CPU.

## Where things live
- **Product context, decisions and roadmap:** Backlog doc-1 (`backlog doc view doc-1`).
- **Technical architecture:** doc-2. **UI design:** doc-3.
- **Work items:** Backlog tasks (`VRT-*`), grouped by milestone (Phase 0-3, Backlog).
- Don't duplicate information. A fact lives in one place (a task, a doc, or this file), and other places link to it.

## How we build this project
- **Stack changes:** stick to the stack in doc-1. Adding a framework or major dependency needs a Backlog decision (`backlog decision create`).
- **Local first and private:** audio, transcripts and voice embeddings never leave the machine. No cloud APIs or telemetry. The LocalAI sidecar binds to `127.0.0.1` only. Voice embeddings are biometric data, so users must always be able to delete them.
- **Spike before you build:** risky unknowns (OS audio APIs, LocalAI endpoints, model behavior) get a Phase 0-style spike first. Record the findings in that task's implementation notes before you build on them.
- **Linux first, cross-platform by design:** develop and verify on Linux (Fedora, PipeWire). Keep platform code behind traits and `cfg` modules (`audio/linux.rs`, `windows.rs`, `macos.rs`), never mixed into shared logic.
- **Audio threads:** realtime callbacks never block, allocate or lock. They only push into lock-free ring buffers (`rtrb`). All other work (encoding, resampling, ASR) runs on consumer threads.
- **One track per source:** mic and system audio are always captured and stored separately. The mic track is "Me". Don't mix down before transcription.
- **Engine abstraction:** all transcription goes through the Rust `Engine` trait. LocalAI-specific code stays in `engine/localai.rs`, so a direct parakeet.cpp FFI engine can replace it later (needed for mobile).
- **Verify the APIs you use:** LocalAI and parakeet-cpp change quickly. Check endpoints and options against the installed version and https://localai.io/docs/features/audio-to-text/, not from memory.
- **IPC:** the UI talks to Rust only through typed wrappers in `src/lib/ipc.ts`. Commands carry requests, and events carry streams (levels, segments).
- **UI:** follow doc-3. It should be calm and minimal, with the transcript as the hero, neutral colors plus one accent, and light and dark modes.

## Testing and quality
- Rust: unit tests for the resampler, the segment merge and bleed logic, and the DB queries. An integration test sends a fixture WAV through the engine.
- UI: Playwright smoke tests.
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
