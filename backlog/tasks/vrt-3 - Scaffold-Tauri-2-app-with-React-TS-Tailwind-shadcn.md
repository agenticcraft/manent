---
id: VRT-3
title: 'Scaffold Tauri 2 app with React, TS, Tailwind, shadcn'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
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
- [ ] #1 npm run tauri dev opens the app on Linux
- [ ] #2 ESLint/Prettier and cargo fmt/clippy configured
- [ ] #3 Design tokens (typography, colors, dark mode) defined and applied to an empty shell with sidebar (Library, Speakers, Settings)
- [ ] #4 Typed IPC helper in src/lib/ipc.ts
- [ ] #5 Project layout matches doc-2 §1.6
<!-- AC:END -->
