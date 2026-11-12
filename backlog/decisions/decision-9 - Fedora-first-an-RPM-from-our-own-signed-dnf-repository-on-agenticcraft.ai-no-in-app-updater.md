---
id: decision-9
title: >-
  Fedora first: an RPM from our own signed dnf repository on agenticcraft.ai, no
  in-app updater
date: '2026-11-04T08:06:00Z'
status: accepted
---
## Context

VRT-23 covers installers, signing and auto-update for three platforms. Windows and macOS still lack their audio code (VRT-11, VRT-12), CI runners (VRT-13) and signing accounts. On 2026-11-04 the user chose to start with the best option for their own Fedora system, with releases hosted on agenticcraft.ai.

Tauri's updater plugin can update RPMs since 2.10. On Fedora, an app updating itself would need root (pkexec) and a signing scheme of its own. Fedora already does both for packages.

## Decision

- **Package:** an **RPM** built by Tauri's bundler (`bundle.targets = ["rpm"]`). `manent` and `manent-engine` go to `/usr/bin` (the helper as an extra RPM file, since `engine::helper::helper_exe` looks next to the app). Icons go to hicolor, plus `Manent.desktop` with `StartupWMClass=manent`. libparakeet and the model are downloaded at first run, as before (decision-1).
- **Updates:** from **our own dnf repository**, served as static files from GitHub Pages at `https://agenticcraft.github.io/manent/rpm` (changed 2026-11-06, VRT-56; first planned on agenticcraft.ai). Users add `manent.repo` once; dnf and GNOME Software then update Manent like any other package. There is no updater code or dependency in the app.
- **Signing:** an "AgenticCraft Releases" GPG key (RSA 4096, 5 years) signs every RPM (`gpgcheck`) and the repository metadata (`repo_gpgcheck`). The public key is published as `RPM-GPG-KEY-agenticcraft` next to the repository. The private key lives outside the repository.
- **Script:** `scripts/release-rpm.sh` builds, signs and updates the repository in `~/.local/share/agenticcraft-release/repo`; `--publish` pushes it to the `gh-pages` branch. Test releases (`--base-url`) go to `repo-test`.
- **Later:**
  - other platforms (AppImage, Windows, macOS), each with the format and update path that suits it, in their own tickets;
  - the Tauri updater only where the platform has nothing of its own.

## Consequences

- Fedora users get signed updates through the system. A broken update can be rolled back with `dnf downgrade`, as for any package.
- The key is a single point of trust: back it up (e.g. `gpg --export-secret-keys` to an offline place). Losing it means publishing a new key and asking users to import it.
- A version bump (tauri.conf.json, Cargo.toml, package.json) is part of each release; dnf only updates to a higher version-release.
