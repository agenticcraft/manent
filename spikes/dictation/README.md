# Spike: push-to-talk dictation (VRT-53)

Probes for dictation on GNOME Wayland (GNOME 51). The findings are in VRT-53's implementation notes.

Everything runs in a **headless GNOME Shell** with its own D-Bus, runtime dir, home and portals (`run-isolated.sh`),
so the probes never touch the user's session, clipboard or settings. Output and screenshots go to `out/` (ignored).

```
spikes/dictation/run-isolated.sh python3 spikes/dictation/<probe>.py
```

- `hands.py`: a virtual keyboard for the test session through Mutter's own RemoteDesktop API and libei (`ei.py`,
  libei over ctypes), screenshots through the Screenshot portal, and a helper for portal requests.
  Mutter's old `NotifyKeyboard*` calls reach the Shell but no app windows; libei (EIS) does.
- `clipboard_background.py`: setting the clipboard while another window has focus.
- `hold_to_talk.py`: GlobalShortcuts Activated/Deactivated while holding a shortcut (accepts GNOME's dialog itself).
- `autopaste.py` (+ `restore.py`, `rd.py`): the RemoteDesktop + Clipboard portals: consent, clipboard, Ctrl+V through EIS,
  restore token after a restart.
- `paste_keys.py [gtk|firefox|chrome|libreoffice|vte|alacritty]`: which paste key works where; `ALSO_PRIMARY=P` also
  sets the primary selection with `wl-copy --primary`.
- `latency.py <manent-engine> <libparakeet.so> <model.gguf> <wav>...`: the engine's transcribe time on short clips.
- `micopen.py`: time from starting a mic capture to its first samples (real session; nothing is stored).

Notes: apps under test get `GTK_IM_MODULE=gtk-im-context-simple`, because GTK's Wayland input method waits for IBus,
which the headless session doesn't run. Ptyxis can't start a shell here (it needs the systemd user instance).
