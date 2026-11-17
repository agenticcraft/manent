# Hold-to-talk through the GlobalShortcuts portal: bind Ctrl+Shift+Space, hold it 1.5 s and tap it, log Activated/Deactivated.
import os, sys, time, subprocess, threading
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
from ei import Keyboard, KEY
from gs import bind
from gi.repository import GLib
SB = os.path.dirname(os.path.abspath(__file__))
h = Hands(); kb = Keyboard(mutter_eis(h))
kb.tap(KEY["esc"]); time.sleep(1)
c, session, events, do_bind = bind()
def accept():
    time.sleep(3)
    for _ in range(3): kb.tap(KEY["tab"]); time.sleep(0.4)
    kb.tap(KEY["enter"])
threading.Thread(target=accept, daemon=True).start()
r = do_bind(); print("bind:", r[0], [(s[0], s[1].get("trigger_description")) for s in r[1].get("shortcuts", [])], flush=True)
marks = []
def hold(sec):
    t0 = time.monotonic(); marks.append(("press", t0))
    for k in ("ctrl", "shift", "space"): kb.key(KEY[k], True)
    end = t0 + sec
    while time.monotonic() < end: kb.pump(0.02)
    for k in ("space", "shift", "ctrl"): kb.key(KEY[k], False)
    marks.append(("release", time.monotonic()))
def run():
    time.sleep(1); hold(1.5); time.sleep(1); hold(0.1); time.sleep(1); loop.quit()
loop = GLib.MainLoop(); threading.Thread(target=run, daemon=True).start(); loop.run()
t0 = marks[0][1]
for name, t in sorted([(m[0], m[1]) for m in marks] + [(e[0], e[2]) for e in events], key=lambda x: x[1]):
    print(f"{(t - t0) * 1000:8.1f} ms  {name}")
