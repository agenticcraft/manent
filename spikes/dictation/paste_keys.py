# Which paste key works where: Ctrl+V, Ctrl+Shift+V, Shift+Insert, via the RemoteDesktop portal.
import os, sys, time, subprocess, threading
sys.path.insert(0, os.path.dirname(__file__))
from hands import *
from ei import Keyboard, KEY
from rd import Portal
from gi.repository import GLib
SB = os.path.dirname(os.path.abspath(__file__))
KEY["insert"] = 110
h = Hands(); hk = Keyboard(mutter_eis(h)); hk.tap(KEY["esc"]); time.sleep(1)
def accept():
    def go():
        time.sleep(3)
        hk.tap(KEY["tab"]); time.sleep(0.3); hk.tap(KEY["space"]); time.sleep(0.3)
        hk.tap(KEY["shift"], KEY["tab"]); time.sleep(0.3); hk.tap(KEY["shift"], KEY["tab"]); time.sleep(0.3)
        hk.tap(KEY["enter"])
    threading.Thread(target=go, daemon=True).start()
p = Portal(on_dialog=accept); kb = Keyboard(p.eis_fd())
ctx = GLib.MainContext.default()
def spin(sec):
    end = time.time() + sec
    while time.time() < end: ctx.iteration(False); time.sleep(0.01)
# GTK's Wayland input method needs IBus, which this headless session doesn't run.
env = {**os.environ, "GTK_IM_MODULE": "gtk-im-context-simple"}
os.makedirs(f"{SB}/out/ff", exist_ok=True)
open(f"{SB}/out/ff/user.js", "w").write("".join(f'user_pref("{k}", {v});\n' for k, v in [
    ("browser.aboutwelcome.enabled", "false"), ("browser.shell.checkDefaultBrowser", "false"),
    ("browser.startup.homepage_override.mstone", '"ignore"'), ("datareporting.policy.dataSubmissionPolicyBypassNotification", "true")]))
apps = {
  "gtk": ["python3", f"{SB}/target.py", f"{SB}/out/target.txt"],
  "firefox": ["firefox", "--new-instance", "--profile", f"{SB}/out/ff", f"file://{SB}/field.html"],
  "vte": ["python3", f"{SB}/vte.py"],
  "alacritty": ["alacritty", "-e", "cat"],
  "chrome": ["google-chrome", f"--user-data-dir={SB}/chrome", "--no-first-run", "--password-store=basic", "--no-default-browser-check", "--ozone-platform=wayland",
             "--app=data:text/html,<textarea autofocus style='width:95vw;height:90vh;font-size:20px'></textarea>"],
  "libreoffice": ["soffice", "--writer", "--norestore", "--nologo", f"-env:UserInstallation=file://{SB}/out/lo"],
}
which = sys.argv[1:] or list(apps)
for name in which:
    proc = subprocess.Popen(apps[name], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    spin(12 if name == "libreoffice" else 6)
    for label, keys in [("C-v", ("ctrl", "v")), ("C-S-v", ("ctrl", "shift", "v")), ("S-Ins", ("shift", "insert"))]:
        p.set_clipboard(f"[{label} é ë “q”] "); spin(0.3)
        if os.environ.get("ALSO_PRIMARY"):
            wp = subprocess.Popen(["wl-copy", "--primary", f"[{label}+P é ë “q”] "]); spin(0.5)
        kb.tap(*[KEY[k] for k in keys]); spin(1.2)
        if name == "libreoffice" and label == "C-S-v":
            subprocess.Popen(["python3", f"{SB}/shot.py", f"{SB}/out/m-{name}-dialog.png"]); spin(2.5)
            kb.tap(KEY["esc"]); spin(0.8)
    subprocess.Popen(["python3", f"{SB}/shot.py", f"{SB}/out/m-{name}{os.environ.get('ALSO_PRIMARY','')}.png"]); spin(2.5)
    proc.terminate(); spin(1)
print("done", flush=True)
