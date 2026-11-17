# Auto-paste end to end: RemoteDesktop portal (keyboard + clipboard), clipboard through the portal, Ctrl+V through its EIS,
# then a restart with the restore token.
import os, sys, time, subprocess, threading
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
from ei import Keyboard, KEY
from rd import Portal
from gi.repository import GLib
SB = os.path.dirname(os.path.abspath(__file__))
h = Hands(); hk = Keyboard(mutter_eis(h))
hk.tap(KEY["esc"]); time.sleep(1)
def accept():
    def go():
        time.sleep(3)
        hk.tap(KEY["tab"]); time.sleep(0.3); hk.tap(KEY["space"]); time.sleep(0.3)
        hk.tap(KEY["shift"], KEY["tab"]); time.sleep(0.3); hk.tap(KEY["shift"], KEY["tab"]); time.sleep(0.3)
        hk.tap(KEY["enter"])
    threading.Thread(target=go, daemon=True).start()
p = Portal(on_dialog=accept)
code, res = p.start
token = res.get("restore_token")
print("start:", code, "devices", res.get("devices"), "clipboard", res.get("clipboard_enabled"), "token", bool(token), flush=True)
kb = Keyboard(p.eis_fd()); print("portal EIS keyboard ok", flush=True)
out = f"{SB}/out/target.txt"; open(out, "w").close()
tgt = subprocess.Popen(["python3", f"{SB}/target.py", out], env={**os.environ, "GTK_IM_MODULE": "gtk-im-context-simple"}); time.sleep(3)
text = "Dit is een dicteertest: é, ë, “quotes” en 'apostrof'.\nTweede regel."
ctx = GLib.MainContext.default()
def spin(sec):
    end = time.time() + sec
    while time.time() < end: ctx.iteration(False); time.sleep(0.01)
t0 = time.monotonic(); p.set_clipboard(text); spin(0.3)
wp = subprocess.Popen(["wl-paste", "-n"], stdout=subprocess.PIPE, text=True)
for _ in range(300):
    spin(0.02)
    if wp.poll() is not None: break
got = wp.stdout.read() if wp.poll() is not None else "<timeout>"
print("clipboard via portal:", "OK" if got == text else f"FAIL {got!r}", flush=True)
t1 = time.monotonic(); kb.tap(KEY["ctrl"], KEY["v"])
for _ in range(100):
    spin(0.02)
    if open(out).read() == text: break
print(f"paste into target: {'OK' if open(out).read() == text else 'FAIL ' + repr(open(out).read())}  set->paste {1000*(time.monotonic()-t0):.0f} ms, ctrl+v->text {1000*(time.monotonic()-t1):.0f} ms", flush=True)
tgt.terminate(); p.close(); time.sleep(1)
# A second session in a new process (like an app restart) with the restore token: no dialog, starts at once.
r = subprocess.run(["python3", f"{SB}/restore.py", token], capture_output=True, text=True, timeout=40)
print(r.stdout.strip() or r.stderr[-400:], flush=True)
time.sleep(1)
