---
id: VRT-58
title: Push-to-talk dictation to the clipboard
status: To Do
assignee: []
created_date: '2026-11-17T09:52:00Z'
updated_date: '2026-11-17T11:20:00Z'
labels:
  - feature
milestone: m-4
dependencies:
  - VRT-53
priority: medium
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Build dictation as chosen in VRT-53 and decision-11. Hold a global shortcut (default Ctrl+Shift+Space, changeable like the other shortcuts, decision-10), speak, release: Manent records the mic only, transcribes it with the local engine and puts the text on the clipboard, and a notification says it is ready to paste. No meeting, no speakers, nothing saved. Off until the user turns it on in Settings > Shortcuts.

From the spike: the portal sends Activated on press (then repeats every 30 ms after 500 ms while held) and one Deactivated on release; the shortcut loop must read both. The mic opens on press (~150 ms to first samples). The clipboard is set with wl-copy (RPM Requires wl-clipboard), which works while another window has focus. Notifications through the Notification portal (ashpd feature notification). Transcription takes ~0.3 s for 5 s of speech and ~1.8 s for 30 s.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Holding the shortcut records the mic and releasing it puts the transcribed text on the clipboard, with Manent in the background; key repeats while holding don't restart it, and a press shorter than 0.3 s does nothing
- [ ] #2 A notification says the text is ready (or that nothing was heard, or that the engine isn't installed); while dictating a calm indicator shows it is listening (tray icon or notification)
- [ ] #3 Dutch and English are transcribed; é, ë, quotes and newlines survive the clipboard
- [ ] #4 Nothing is stored: no meeting, no audio file, no speaker embedding; the audio stays in memory
- [ ] #5 Settings > Shortcuts turns dictation on and off and shows its key; the choice survives a restart
- [ ] #6 Unit tests for the hold logic; a smoke script covers dictation to the clipboard; screenshots of the settings in light and dark
- [ ] #7 With the RPM, wl-clipboard is installed as a dependency; without wl-copy (a development build) dictation says so instead of failing silently
- [ ] #8 During a meeting recording the dictation shortcut does nothing but show a notification that dictation isn't available while recording (decision-11)
<!-- AC:END -->
