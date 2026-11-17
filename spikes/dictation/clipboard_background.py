# Can an app set the clipboard while another window has focus? (wl-copy, xclip, GTK3/GTK4 on Wayland and X11)
import os, subprocess, sys, time, glob
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
SB = os.path.dirname(os.path.abspath(__file__)); RT = os.environ["XDG_RUNTIME_DIR"]
os.environ["XAUTHORITY"] = glob.glob(f"{RT}/.mutter-Xwaylandauth.*")[0]
from ei import Keyboard, KEY
h = Hands(); kb = Keyboard(mutter_eis(h))
out = f"{SB}/out/target.txt"
tgt = subprocess.Popen(["python3", f"{SB}/target.py", out]); time.sleep(4)
kb.tap(KEY["esc"]); time.sleep(1.5)
def paste(): return subprocess.run(["wl-paste", "-n"], capture_output=True, text=True, timeout=5).stdout
def mark(s): print(f"--- {s} @ {time.time():.3f}", flush=True)
cases = [
  ("wl-copy (focus trick)", lambda t: subprocess.Popen(["wl-copy", "--foreground", t])),
  ("xclip (X11/XWayland)", lambda t: subprocess.Popen(["xclip", "-selection", "clipboard", "-loops", "3"], stdin=subprocess.PIPE, text=True)),
  ("GTK3 Wayland, no focus", lambda t: subprocess.Popen(["python3", f"{SB}/clipset_gtk.py", "3.0", t])),
  ("GTK3 X11, no focus", lambda t: subprocess.Popen(["python3", f"{SB}/clipset_gtk.py", "3.0", t], env={**os.environ, "GDK_BACKEND": "x11"})),
  ("GTK4 Wayland, no focus", lambda t: subprocess.Popen(["python3", f"{SB}/clipset_gtk.py", "4.0", t])),
]
for i, (name, start) in enumerate(cases):
    t = f"case{i} é ë “q”"
    mark(name); p = start(t)
    if p.stdin: p.stdin.write(t); p.stdin.close()
    time.sleep(1.5)
    got = paste()
    print(f"{name}: {'OK' if got == t else 'FAIL'} clipboard={got!r}", flush=True)
    p.kill(); time.sleep(0.5)
tgt.terminate()
