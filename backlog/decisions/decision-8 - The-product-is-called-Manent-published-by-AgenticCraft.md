---
id: decision-8
title: The product is called Manent, published by AgenticCraft
date: '2026-11-03T19:24:00Z'
status: accepted
---
## Context

"Veritas" was the working name. A naming round on 2026-11-03 found these reasons against it:
- **Trademark:** Veritas Technologies is a large software company in data storage and backup, the same class as an app that stores and syncs recordings.
- **Findability:** the name is used widely (Veritas Capital, Veritas Genetics, Harvard's motto, many apps), so it can't be found in search or the app stores.
- **The promise:** "truth" overstates what a transcript is (10–15% of the words wrong or missing) and sounds legal or evidential for a meeting recorder.
- **The identifier:** `nl.veritas.app` points at a domain outside the project.

## Decision

- **The name:** the product is **Manent**, after *verba volant, scripta manent*: spoken words fly away, written ones remain. That is what the app does.
- **The publisher:** **AgenticCraft**, the studio name. "Manent, by AgenticCraft." The product name doesn't carry "agentic": the app listens and writes things down locally, and it isn't an agent.
- **Web:** under agenticcraft.ai, e.g. `manent.agenticcraft.ai` or `agenticcraft.ai/manent`. No separate domain.
- **App identifier:** `ai.agenticcraft.manent`, the reverse of the publisher's domain.
- **Kept as they are:** the Backlog task prefix `VRT` and the repository name, as history. The spikes keep their old names.
- **Before a release:** check the trademark registers (BOIP/EUIPO, TMview: "MANENT" in classes 9 and 42).

## Consequences

- Rename in code, UI and docs (VRT-44): crate and binary names, the engine helper (`manent-engine`), environment variables (`MANENT_*`), the UI strings, the tray, and the docs.
- **Data migration**, because the identifier sets the app's data folder:
  - On the first start, an existing `nl.veritas.app` data folder moves to the new one, and the database paths that point into it are rewritten.
  - The Nextcloud app password moves to the new keychain service.
  - A sync connection keeps its "Veritas" server folder.
  - `transcript.md` is written with `manent: 1`, and the parser still reads `veritas: 1`.
