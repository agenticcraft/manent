---
id: VRT-59
title: 'Option: paste dictated text automatically'
status: To Do
assignee: []
created_date: '2026-11-17T09:52:00Z'
updated_date: '2026-11-17T10:23:00Z'
labels:
  - feature
milestone: m-4
dependencies:
  - VRT-58
priority: low
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A setting, off by default, under the dictation setting (built on VRT-58, findings in VRT-53, decision-11): after dictation Manent pastes the text where the cursor is. It uses the RemoteDesktop portal with the Clipboard portal: the text goes on the clipboard through the portal and Ctrl+V goes through the portal's EIS connection (libei client: the reis crate; Mutter's NotifyKeyboard* calls don't reach app windows). GNOME asks once; in that dialog 'Allow Remote Interaction' is off by default and must be switched on, so the settings text says so. The restore token comes back new after every start and must be stored each time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With the setting on, released dictation is pasted into the focused field (browser field, LibreOffice); GNOME asks only the first time, also after a restart
- [ ] #2 Turning the setting off ends the portal session and forgets the restore token; if the user refuses the portal, the setting turns itself off with a short explanation and dictation keeps working via the clipboard
- [ ] #3 Smoke script and screenshots in light and dark
- [ ] #4 The paste key is a setting: Ctrl+V (default; GTK apps, browsers, LibreOffice) or Ctrl+Shift+V (terminals); checked on a real desktop in Ptyxis, Firefox and LibreOffice
- [ ] #5 The user's previous clipboard is restored after the paste (read through the Clipboard portal), or the reason it isn't is documented
<!-- AC:END -->
