---
id: VRT-3
title: Scaffold Tauri 2 app with React, TS, Tailwind, shadcn
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T21:25:00Z'
labels:
  - setup
  - frontend
milestone: m-1
dependencies: []
documentation:
  - doc-2
  - doc-3
priority: high
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the app skeleton: Tauri 2 (Rust) + React + TypeScript + Vite + Tailwind + shadcn/ui, with lint/format and the clean design foundation (Inter, zinc neutrals, single accent, light/dark following OS, thin sidebar layout). See doc-2 Technical architecture §1.1, §1.6 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 npm run tauri dev opens the app on Linux
- [x] #2 ESLint/Prettier and cargo fmt/clippy configured
- [x] #3 Design tokens (typography, colors, dark mode) defined and applied to an empty shell with sidebar (Library, Speakers, Settings)
- [x] #4 Typed IPC helper in src/lib/ipc.ts
- [x] #5 Project layout matches doc-2 §1.6
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Scaffold Tauri 2 + React + TypeScript + Vite (create-tauri-app, react-ts) in a temp dir and move it to the repo root, matching doc-2 §1.6 (src/, src-tauri/).
2. Tailwind CSS v4 (@tailwindcss/vite) + shadcn/ui (zinc base); '@/' path alias.
3. Design tokens per doc-3 in src/index.css: Inter bundled locally (@fontsource-variable, no CDN - local-first), zinc neutrals, one accent, recording red, light/dark following the OS (matchMedia toggles the dark class).
4. App shell: thin sidebar (Library, Speakers, Settings) + empty route components in src/routes; components/ for shared UI.
5. Typed IPC helper src/lib/ipc.ts (typed invoke + listen maps); one Rust command (app_info) to prove the round trip.
6. Rust module skeleton in src-tauri/src: audio/, engine/, speakers.rs, db.rs (doc comments only).
7. ESLint (flat config, typescript-eslint, react-hooks) + Prettier; rustfmt.toml; cargo clippy clean.
8. Verify: tsc + eslint + prettier --check + cargo fmt --check + clippy; run the app and inspect it with tauri-driver/WebKitWebDriver (DOM check + screenshot in light and dark).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scaffolded with create-tauri-app (react-ts) and moved to the repo root. Removed the opener plugin and demo assets. Tauri 2 puts setup in lib.rs (main.rs is a thin entry); added commands.rs; doc-2 §1.6 updated.
Frontend: React 19, TS 6, Vite 8, Tailwind 4 (@tailwindcss/vite), shadcn/ui (radix base, Nova preset; theme replaced by doc-3 tokens). shadcn now depends on the 'cn' npm package for class merging: verified as shadcn's own (github.com/shadcn-ui/cn, MIT). Geist font removed; Inter bundled locally via @fontsource-variable (no CDN, local-first). TS 6 deprecates baseUrl: '@/' alias uses paths only.
Design tokens (src/index.css): zinc neutrals, one indigo accent (--brand), --recording red, --caption grey, 8 pastel --speaker-N colors, .transcript-text, .recording-pulse; dark mode via .dark class synced to prefers-color-scheme (src/lib/theme.ts).
Security: CSP restricted to self + Tauri IPC; capabilities only core:default; unsafe_code forbidden.
Verification:
- npm run check (tsc, ESLint, Prettier) clean; cargo fmt --check clean; cargo clippy --all-targets 0 warnings; cargo test 2/2 (app_info).
- npm run tauri dev: Vite on localhost:1420 (IPv6) and the app process stays up.
- scripts/ui-smoke.py via tauri-driver + WebKitWebDriver on the real debug build: 3/3 runs pass (sidebar items, headings and active nav per page, Inter font, theme follows OS, IPC app_info shown as 'Veritas 0.1.0 · linux/x86_64'); screenshots of all pages in light and dark inspected and match doc-3.

Verification (2026-10-29):
- npm run check (tsc, ESLint, Prettier) passes; in src-tauri: cargo fmt --check, cargo clippy --all-targets -D warnings and cargo test (2 tests: app_info version + JSON field names) pass.
- Real app: npm run tauri build -- --debug --no-bundle, then tauri-driver + WebKitWebDriver and python3 scripts/ui-smoke.py src-tauri/target/debug/veritas <dir>: PASS. It checks the sidebar (Library, Speakers, Settings), each page heading and active nav, Inter as body font, the theme following the OS (.dark when prefers-color-scheme is dark), and the IPC round trip (Settings > About shows 'Veritas 0.1.0 · linux/x86_64' from the app_info command). Screenshots in light and dark reviewed.
- The 'New recording' button in Library is disabled on purpose (dimmed) until VRT-8.
Layout vs doc-2 §1.6: src-tauri/src has audio/ (mod, linux, windows, macos), engine/ (mod, localai), speakers.rs, db.rs, commands.rs; Tauri 2 puts the setup in lib.rs with a thin main.rs. Not created yet, by design: src-tauri/binaries/ (VRT-14), routes Live and Meeting (VRT-8, VRT-20) and the components TranscriptView, SpeakerChip, LevelMeter, SourcePicker (their feature tasks).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
App skeleton in place: Tauri 2 + React + TypeScript + Vite, Tailwind v4 and shadcn/ui (zinc), Inter bundled locally, zinc neutrals with one indigo accent and a recording red, light/dark following the OS. Thin sidebar shell with Library, Speakers and Settings routes, a typed IPC helper (src/lib/ipc.ts) and one Rust command (app_info) proving the round trip, plus doc-commented module stubs for audio, engine, speakers and db. ESLint/Prettier and rustfmt/clippy configured. Verified with npm run check, cargo fmt/clippy/test and the tauri-driver smoke test (scripts/ui-smoke.py, PASS, screenshots reviewed in light and dark). Follow-ups live in their own tasks: LocalAI sidecar binaries (VRT-14), Live/Meeting routes and transcript components (VRT-8, VRT-20).
<!-- SECTION:FINAL_SUMMARY:END -->
