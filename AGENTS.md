# Manent: agent instructions

Manent is a local-first desktop app that records meetings (the mic and system audio as separate tracks) and transcribes them live with speaker identification, using parakeet.cpp (`libparakeet`) on the CPU.

## Where things live
- **Product context, decisions and roadmap:** Backlog doc-1 (`backlog doc view doc-1`).
- **Technical architecture:** doc-2. **UI design:** doc-3.
- **Work items:** Backlog tasks (`VRT-*`), grouped by milestone (Phase 0-3, Backlog).
- Don't duplicate information. A fact lives in one place (a task, a doc, or this file), and other places link to it.

## How we build this project
- **Stack changes:** stick to the stack in doc-1. Adding a framework or major dependency needs a Backlog decision (`backlog decision create`).
- **Local first and private:** audio, transcripts and voice embeddings never leave the machine. No cloud APIs or telemetry. The engine and model downloads are the only internet traffic; the engine helper talks to the app over stdin/stdout, and the media server binds to `127.0.0.1` only. Two exceptions are planned, both off by default and revocable: the mobile companion service (VRT-26.2, decision-4: local network, paired devices only, encrypted, as decided in VRT-26.1), and sync with the user's own Nextcloud folder over WebDAV, built into the app (decision-5 and decision-7: Login Flow v2, the app password only in the OS keychain, revoked on disconnect; VRT-39). Neither ever carries voice embeddings or the database. Voice embeddings are biometric data, so users must always be able to delete them.
- **Spike before you build:** risky unknowns (OS audio APIs, engine APIs, model behavior) get a Phase 0-style spike first. Record the findings in that task's implementation notes before you build on them.
- **Linux first, cross-platform by design:** develop and verify on Linux (Fedora, PipeWire). Keep platform code behind traits and `cfg` modules (`audio/linux.rs`, `windows.rs`, `macos.rs`), never mixed into shared logic.
- **Audio threads:** realtime callbacks never block, allocate or lock. They only push into lock-free ring buffers (`rtrb`). All other work (encoding, resampling, ASR) runs on consumer threads.
- **One track per source:** mic and system audio are always captured and stored separately. In a call the mic track is "Me"; in the room (the Live screen's switch, VRT-47) it is diarized like the system track. Don't mix down before transcription.
- **Engine abstraction:** all transcription goes through the Rust `Engine` trait. The libparakeet FFI lives only in the `manent-engine` helper process (the only crate that may use `unsafe`), so an engine crash never stops a recording. Phones don't transcribe: the mobile companion delegates to the user's desktop (decision-4), directly or through Nextcloud (decision-5).
- **Verify the APIs you use:** parakeet.cpp changes quickly. Check functions and options against the pinned `parakeet_capi.h` (ABI v10, `spikes/engine/include/`), not from memory.
- **IPC:** the UI talks to Rust only through typed wrappers in `src/lib/ipc.ts`. Commands carry requests, and events carry streams (levels, segments).
- **UI:** follow doc-3. It should be calm and minimal, with the transcript as the hero, neutral colors plus one accent, and light and dark modes.

## Testing and quality
- **Rust:** unit tests next to the code (resampler, chunker, segment merge, bleed and labels, speaker matching, DB queries, downloads, import decoding). Integration tests that run fixture WAVs (`src-tauri/tests/fixtures`) through the real engine are `#[ignore]`d; run them with a data folder that has the engine and model downloaded: `MANENT_TEST_ENGINE_DATA=<data dir> cargo test --workspace -- --ignored engine_tests --test-threads 1`.
- **Checks** (in `src-tauri/`): `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Frontend: `npm run check` (tsc, ESLint, Prettier).
- **UI:** smoke-test the real app, not just a browser. Build with `npm run tauri build -- --debug --no-bundle` (this also builds the engine helper), start `tauri-driver` (`cargo install tauri-driver --locked`; needs `WebKitWebDriver`) with `MANENT_DATA_DIR=<temp folder>`, so tests never touch the real app data, then run the scripts in `scripts/` (each documents its arguments at the top):
  - Without audio hardware or engine: `ui-smoke.py`, `smoke-library.py`.
  - Recording (audio hardware): `smoke-live.py`, `smoke-playback.py`, `smoke-settings.py` (briefly switches the OS default mic and restores it), `smoke-awake.py` (sleep blocked while recording, released on stop and crash; the audio-gap marker). `smoke-marks.py` (marks from the Mark button and the shortcut's path, labels, deletes, player ticks; Settings > Shortcuts, with `--bind` turned on and off, which needs `ai.agenticcraft.manent.desktop` installed, decision-10). `smoke-tray.py` (tray menu over D-Bus: start/stop, icon, close-to-tray, quit during a recording; the close check needs tauri-driver started with `GDK_BACKEND=x11`).
  - Engine: `smoke-engine.py` (download, cancel/resume, start, crash restart; downloads ~56 MB with `--seed-model <local gguf>`); it leaves the engine in the data folder for the scripts below.
  - Transcription (engine + audio hardware; play loudness-normalised speech with its `.truth.json` next to it): `smoke-captions.py` (live captions: latency, WER per language), `smoke-speakers.py` (rolling and final pass, labels per voice, bleed), `smoke-naming.py` (naming, recognition after a restart, merge/delete). `smoke-recover.py` (kills the app during a recording; the next start recovers, plays and transcribes it).
  - Engine only: `smoke-import.py` (imports in several formats, re-transcribe), `smoke-transcript.py` (search, open at a match, click-to-seek, playback highlight) `smoke-export.py` (all export formats) and `smoke-transcript-md.py` (transcript.md backfill, contents, rewrite after changes); these need transcribed meetings. `smoke-storage.py` (update interval, check/remove engine, moving the meetings folder, keeping only the playback file; set `TAURI_DRIVER_LOG` to the driver's log to check the interval).
  - Nextcloud sync: `smoke-sync.py` (engine + a local Nextcloud container with a test user, see `spikes/nextcloud/e2e.py`; start tauri-driver with `MANENT_NO_BROWSER=1`): connect, inbox import with meta.json, transcript upload, all meetings, deletes, disconnect.
  - Rename: `smoke-rename.py` (an install as Veritas left it, in a temp `XDG_DATA_HOME`, is carried over: data folder, database paths, transcript.md, Nextcloud keychain entry).
  - `screens.py` takes a screenshot of every screen in light and dark, at the default and the minimum window size, for design reviews (VRT-42).
  - Extend the scripts when a ticket adds UI, and look at the screenshots (light and dark).
- Before you mark a task done, `cargo fmt`, `cargo clippy`, the lint, and the tests must all pass, and every acceptance criterion must be checked.

## Releases
- **Fedora (decision-9):** `scripts/release-rpm.sh` builds the RPM (app + engine helper), signs it and updates the dnf repository in `$RELEASE_HOME` (default `~/.local/share/agenticcraft-release`, holding the signing key, never in this repository); upload `repo/` as-is to `https://agenticcraft.ai/manent/rpm`. Test a release first with `--version X.Y.Z --base-url http://127.0.0.1:8472`, serving `repo/` with `python3 -m http.server 8472 --bind 127.0.0.1`, then `dnf upgrade --refresh manent`. Bump the version in `tauri.conf.json`, `Cargo.toml` and `package.json` for a real release.
- macOS and Windows installers: VRT-50, VRT-51; CI: VRT-13.

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
