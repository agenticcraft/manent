"""Tray icon smoke check on the real app (VRT-22). Linux with a StatusNotifier/AppIndicator host.

Usage: python3 scripts/smoke-tray.py <app binary> <data dir> <screenshot dir>

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (audio hardware for the recording). The
tray menu is driven over D-Bus (com.canonical.dbusmenu, as the panel does): "Start recording"
starts a recording and the window follows; the icon and the menu show it; the window's own Stop
button updates the tray; "Stop recording" stops one started in the window; closing the window
while recording hides it and the recording goes on; "Show Veritas" brings it back.
"""
import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
ITEM = "/org/ayatana/NotificationItem/tray_icon_tray_app_main"


def busctl(*args):
    out = subprocess.run(["busctl", "--user", "--json=short", "--", *args], capture_output=True, text=True)
    return json.loads(out.stdout) if out.returncode == 0 and out.stdout.strip() else None


def tray_bus():
    pid = subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True, text=True).stdout.split()
    names = subprocess.run(["busctl", "--user", "--list", "--no-pager"], capture_output=True, text=True).stdout
    for line in names.splitlines():
        parts = line.split()
        if len(parts) > 1 and parts[1] in pid and parts[0].startswith(":"):
            tree = subprocess.run(["busctl", "--user", "tree", parts[0], "--list"], capture_output=True, text=True).stdout
            if ITEM in tree:
                return parts[0]
    return None


def menu(bus):
    """{label: id} of the tray menu."""
    layout = busctl("call", bus, ITEM + "/Menu", "com.canonical.dbusmenu", "GetLayout", "iias", "0", "-1", "0")
    items = {}
    for child in layout["data"][1][2]:
        node = child["data"]
        label = node[1].get("label", {}).get("data")
        if label:
            items[label.replace("_", "")] = node[0]
    return items


def click(bus, label):
    items = menu(bus)
    subprocess.run(["busctl", "--user", "call", bus, ITEM + "/Menu", "com.canonical.dbusmenu", "Event",
                    "isvu", str(items[label]), "clicked", "s", "", "0"], check=True)


def icon(bus):
    """The icon's pixels (each change is written to a new file, so compare the content)."""
    v = busctl("get-property", bus, ITEM, "org.kde.StatusNotifierItem", "IconName")
    try:
        return v and Path(v["data"]).read_bytes()
    except OSError:
        return None


def wait_until(cond, timeout=10.0):
    end = time.time() + timeout
    while time.time() < end:
        if cond():
            return True
        time.sleep(0.2)
    return cond()


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
bus = None
wait_until(lambda: (globals().__setitem__("bus", tray_bus()) or bus) is not None, 10)
s.check(bus is not None, f"tray registered on the session bus ({bus})")
s.check(set(menu(bus)) == {"Start recording", "Show Veritas", "Quit Veritas"}, f"menu: {sorted(menu(bus))}")
idle_icon = icon(bus)

# Start from the tray: the window follows and the tray shows it.
click(bus, "Start recording")
live = s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 15)
s.check(bool(live), "tray 'Start recording' starts a recording; the window shows it")
s.check(wait_until(lambda: "Stop recording" in menu(bus)), f"menu now offers 'Stop recording' ({sorted(menu(bus))})")
s.check(wait_until(lambda: icon(bus) not in (None, idle_icon)), "icon shows the recording")
time.sleep(2)

# Stop from the tray.
click(bus, "Stop recording")
back = s.wait_for("return document.querySelector('[data-testid=new-recording]') && !document.querySelector('[data-testid=recording-timer]')", 30)
s.check(bool(back) and s.invoke("current_recording") is None, "tray 'Stop recording' stops it; the window goes back to the Library")
s.check(wait_until(lambda: "Start recording" in menu(bus) and icon(bus) == idle_icon), "tray back to idle")

# Started in the window, stopped in the window: the tray follows.
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 15)
s.check(wait_until(lambda: "Stop recording" in menu(bus)), "the tray follows a recording started in the window")
s.js("document.querySelector('[data-testid=meeting-title-input]').focus()")
s.js("const el = document.querySelector('[data-testid=meeting-title-input]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, 'Typed, not saved');"
     "el.dispatchEvent(new Event('input', {bubbles: true}));")
meeting = s.invoke("current_recording")["meeting_id"]
time.sleep(1)
click(bus, "Stop recording")  # while the title field still has focus
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 30)
s.check(wait_until(lambda: s.invoke("get_meeting", {"id": meeting})["title"] == "Typed, not saved"),
        "a title being typed is kept when the tray stops the recording")
s.screenshot(OUT / "tray-after-stop.png")

# Closing the window while recording hides it; the recording goes on; "Show Veritas" brings
# it back. The close is asked like the title bar button does (_NET_CLOSE_WINDOW via wmctrl), so
# this part needs the app on X11: start tauri-driver with GDK_BACKEND=x11. (WebDriver's own
# Close Window destroys the webview directly, which is not what a user does.)
click(bus, "Start recording")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 15)
meeting = s.invoke("current_recording")["meeting_id"]


def app_windows():
    pids = subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True, text=True).stdout.split()
    out = subprocess.run(["wmctrl", "-l", "-p"], capture_output=True, text=True).stdout
    return [line.split()[0] for line in out.splitlines() if len(line.split()) > 2 and line.split()[2] in pids]


if app_windows():
    subprocess.run(["wmctrl", "-i", "-c", app_windows()[0]], check=True)
    hidden = wait_until(lambda: not app_windows(), 5)
    alive = bool(subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True).stdout)
    s.check(hidden and alive and s.invoke("current_recording") is not None,
            "closing the window hides it; the app and the recording keep running")
    click(bus, "Show Veritas")
    s.check(wait_until(lambda: bool(app_windows()), 5), "'Show Veritas' brings the window back")
else:
    print("     (window close not checked: the app is not on X11; see the comment above)")
time.sleep(1)

# Quit while recording: the recording is finished first (complete files, then the final pass).
click(bus, "Quit Veritas")
gone = wait_until(lambda: not subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True).stdout, 30)
con = sqlite3.connect(DATA / "veritas.db")
status, duration = con.execute("SELECT status, duration_ms FROM meetings WHERE id = ?", (meeting,)).fetchone()
seconds = [x for (x,) in con.execute("SELECT seconds FROM tracks WHERE meeting_id = ?", (meeting,))]
con.close()
s.check(gone and status in ("processing", "done") and duration > 1000 and seconds and all(x > 1 for x in seconds),
        f"'Quit Veritas' finishes the recording first ({status}, {duration} ms, tracks {seconds})")
try:
    s.close()
except RuntimeError:
    pass  # the app is gone
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
