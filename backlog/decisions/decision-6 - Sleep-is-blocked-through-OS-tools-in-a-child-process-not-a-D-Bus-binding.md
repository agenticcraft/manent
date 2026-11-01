---
id: decision-6
title: Sleep is blocked through OS tools in a child process, not a D-Bus binding
date: '2026-11-01T16:06:00Z'
status: accepted
---
## Context

In VRT-35 the laptop went into idle suspend twice during a 25-minute recording. Capture froze, and minutes of the meeting were lost. VRT-36 must keep the computer awake while recording.

On Linux this means a logind inhibitor. The obvious route is a D-Bus client such as zbus. That would be a new, fairly large dependency, which needs a decision (AGENTS.md). zbus is not in the dependency tree yet.

## Decision

- **Linux:** run `systemd-inhibit --what=sleep --mode=block --who=Veritas --why="Recording a meeting" cat` as a child process for the length of the recording.
  - `cat` waits on the app's stdin pipe. If the app exits or crashes, the pipe closes and the inhibitor ends with it.
  - On stop, the app kills the child.
  - Only `sleep` is blocked, not `idle`, so the screen still locks and blanks as usual.
- **macOS:** `caffeinate -i -w <app pid>` holds a "prevent idle system sleep" assertion until the app exits. Not yet verified on a Mac (VRT-12).
- **Windows:** `SetThreadExecutionState` needs a system API call, which means `unsafe`. Only the engine helper may use `unsafe`, so this is deferred to the Windows work (VRT-11). Until then, Windows holds no block and logs why.
- **Missing tool:** the recording goes on, and the log says so.
- **Lid close:** closing the lid still suspends; that is the user's choice. The recorder notices the frozen stretch, because wall-clock time passed without audio, and the meeting shows where the audio is missing.

## Consequences

- No new crate. The code lives in `src-tauri/src/power/`, one `cfg` module per platform.
- It depends on `systemd-inhibit` being present, which is the case on systemd distributions such as Fedora, Ubuntu, Debian and Arch. Distributions without systemd don't get the block. Flatpak packaging would need the XDG Inhibit portal instead (VRT-23).
- Verified on Fedora 44 (GNOME): logind lists the inhibitor during a recording and drops it on stop and when the app is killed.
