"""Dictation smoke check (VRT-58): hold the shortcut, speak, release, text on the clipboard.

Usage (from the repository root):
  PIPEWIRE_RUNTIME_DIR=$XDG_RUNTIME_DIR spikes/dictation/run-isolated.sh \\
    python3 scripts/smoke-dictation.py <app binary> <data dir> <screenshot dir> <engine data dir>

Runs inside the headless GNOME session of spikes/dictation (its own D-Bus, clipboard, portals
and settings), so the user's session and clipboard are never touched. Audio goes through the
user's PipeWire, but only through a temporary virtual mic (pw-loopback) that this script feeds
with the speech fixtures; the default devices and the speakers are left alone.

<data dir> must be an empty temp folder; <engine data dir> holds the downloaded engine and model
(they are linked into the data folder halfway, read-only). Starts tauri-driver itself.

Checks: Settings > Shortcuts > Dictation (screenshots light and dark), turning it on (GNOME's
dialog is accepted with the virtual keyboard), the engine-missing notice, a tap that does
nothing, Dutch and English dictation to the clipboard, nothing heard, nothing stored, the
block during a meeting recording, and dictation still on after a restart.

Auto-paste (VRT-59): refusing GNOME's Remote Desktop dialog turns the setting off with a
reason; accepting it (Allow Remote Interaction switched on) types into a GTK text field, after
the user lets go of Ctrl+Shift, and leaves the clipboard alone; it types into a terminal
(alacritty); with a Russian layout (latin letters can't be typed) the letters are pasted and
the previous clipboard text is put back; after a restart it types without asking; turning
it off forgets the token.
"""
import os
import signal
import sqlite3
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "spikes" / "dictation"))
from wdlib import Session  # noqa: E402
from hands import Hands, mutter_eis, shot  # noqa: E402
from ei import Keyboard, KEY  # noqa: E402

APP, DATA, OUT, ENGINE_DATA = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
FIXTURES = ROOT / "src-tauri" / "tests" / "fixtures"
SPEECH = {
    "nl": (FIXTURES / "nl-fleurs-1784.wav", ["groenland", "ijsland", "naam"]),
    "en": (FIXTURES / "en-fleurs-1652.wav", ["religion", "moldova", "christian"]),
}
OUT.mkdir(parents=True, exist_ok=True)
DATA.mkdir(parents=True, exist_ok=True)
LOG = OUT / "app.log"

# A timeout's SIGTERM still runs the cleanup below.
signal.signal(signal.SIGTERM, lambda *_: sys.exit(2))

# The desktop knows the app by its desktop file (decision-10).
apps = Path(os.environ["XDG_DATA_HOME"]) / "applications"
apps.mkdir(parents=True, exist_ok=True)
(apps / "ai.agenticcraft.manent.desktop").write_text(
    "[Desktop Entry]\nType=Application\nName=Manent\nExec=/bin/true\nNoDisplay=true\n"
)

# A virtual mic: what is played into manent_smoke_in comes out of the source manent_smoke_mic.
loopback = subprocess.Popen(
    ["pw-loopback", "-n", "manent-smoke", "--channels", "1",
     "--capture-props=media.class=Audio/Sink node.name=manent_smoke_in node.description=Manent-smoke-in",
     "--playback-props=media.class=Audio/Source node.name=manent_smoke_mic node.description=Manent-smoke-mic"]
)
driver = subprocess.Popen(
    ["tauri-driver"], env={**os.environ, "MANENT_DATA_DIR": str(DATA)},
    stdout=open(LOG, "w"), stderr=subprocess.STDOUT,
)
failures = []


def log_lines(after):
    return [l for l in LOG.read_text(errors="replace").splitlines()[after:] if l.startswith(("dictation:", "shortcut:"))]


def log_len():
    return len(LOG.read_text(errors="replace").splitlines())


def wait_log(after, needle, timeout=20):
    end = time.time() + timeout
    while time.time() < end:
        hit = [l for l in log_lines(after) if needle in l]
        if hit:
            return hit[0]
        time.sleep(0.2)
    return None


def clipboard():
    r = subprocess.run(["wl-paste", "--no-newline"], capture_output=True, text=True, timeout=5)
    return r.stdout if r.returncode == 0 else ""


def hold(seconds, wav=None, linger=0.0):
    """Hold Ctrl+Shift+Space for `seconds`, playing `wav` into the virtual mic meanwhile; with
    `linger`, Ctrl+Shift stay down that long after Space is let go."""
    for k in ("ctrl", "shift", "space"):
        kb.key(KEY[k], True)
    t0 = time.monotonic()
    player = None
    if wav:
        kb.pump(0.4)  # the mic opens on the press
        player = subprocess.Popen(["pw-play", "--target", "manent_smoke_in", str(wav)])
    while time.monotonic() - t0 < seconds:
        kb.pump(0.02)
    if player:
        player.wait()
    kb.key(KEY["space"], False)
    kb.pump(max(linger, 0.02))
    for k in ("shift", "ctrl"):
        kb.key(KEY[k], False)
    kb.pump(0.05)


def themed(s, name):
    s.js("document.querySelector('[data-testid=dictation-settings]').scrollIntoView({block: 'center'})")
    for dark in (True, False):
        s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
        time.sleep(0.4)
        s.screenshot(OUT / f"{name}-{'dark' if dark else 'light'}.png")


def setting(key):
    con = sqlite3.connect(DATA / "manent.db")
    row = con.execute("SELECT value FROM settings WHERE key = ?", (key,)).fetchone()
    con.close()
    return row[0] if row else None


def accept_remote_desktop():
    """GNOME's Remote Desktop dialog: switch on Allow Remote Interaction, then Share."""
    time.sleep(3)
    shot(str(OUT / "gnome-remote-desktop-dialog.png"))
    kb.tap(KEY["tab"])
    time.sleep(0.3)
    kb.tap(KEY["space"])
    time.sleep(0.3)
    for _ in range(2):
        kb.tap(KEY["shift"], KEY["tab"])
        time.sleep(0.3)
    kb.tap(KEY["enter"])


def target(cmd, out):
    """A window that writes what is typed or pasted into it to `out`."""
    out.write_text("")
    proc = subprocess.Popen(cmd, env={**os.environ, "GTK_IM_MODULE": "gtk-im-context-simple"},
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(4)
    return proc


def paste_into(s, label, cmd, out, expect, outcome, enter=False, linger=0.0):
    """Dictate English into the window `cmd` opens; `expect` is a word, or the whole text."""
    proc = target(cmd, out)
    subprocess.run(["wl-copy", "earlier clipboard é"], check=True)
    n = log_len()
    hold(0.5, SPEECH["en"][0], linger)
    pasted = wait_log(n, "dictation: pasted", 90)
    if enter:
        time.sleep(0.5)
        kb.tap(KEY["enter"])
    time.sleep(1)
    got = out.read_text(errors="replace").strip()
    ok = got == expect if " " in expect else expect in got.lower()
    s.check(bool(pasted) and f"({outcome})" in pasted and ok, f"{label}: {got!r} ({pasted})")
    proc.terminate()
    return got if pasted else None


def layout(name):
    subprocess.run(["gsettings", "set", "org.gnome.desktop.input-sources", "sources",
                    f"[('xkb', '{name}')]"], check=True)
    # The desktop replaces the EIS keyboards with ones for the new layout, a few seconds later.
    resumed = kb.log.count("EI_EVENT_DEVICE_RESUMED")
    end = time.monotonic() + 20
    while kb.log.count("EI_EVENT_DEVICE_RESUMED") == resumed and time.monotonic() < end:
        kb.pump(0.1)
    kb.pump(1)
    print(f"     layout {name}: keyboard replaced after "
          f"{20 - (end - time.monotonic()):.1f} s", flush=True)


def open_settings(s):
    s.wait_for("return [...document.querySelectorAll('nav ul button')].find(b => b.textContent.trim() == 'Settings')", 20)
    s.js("[...document.querySelectorAll('nav ul button')].find(b => b.textContent.trim() == 'Settings').click()")
    return s.wait_for("return document.querySelector('[data-testid=dictation-settings]')", 10)


def stored():
    con = sqlite3.connect(DATA / "manent.db")
    meetings = con.execute("SELECT COUNT(*) FROM meetings").fetchone()[0]
    con.close()
    files = [p.name for p in DATA.rglob("*") if p.is_file() and not p.name.startswith("manent.db")
             and "engine" not in p.parts and "models" not in p.parts]
    runtime = [p.name for p in Path(os.environ["XDG_RUNTIME_DIR"]).glob("manent-dictation-*")]
    return meetings, files, runtime


try:
    time.sleep(2)
    print("virtual keyboard…", flush=True)
    hands = Hands()  # keep the Mutter session alive
    kb = Keyboard(mutter_eis(hands))
    kb.tap(KEY["esc"])  # leave the overview the session starts in
    time.sleep(1)
    print("app…", flush=True)
    s = Session(APP)
    s.check(bool(open_settings(s)), "Settings shows the dictation section")
    row = s.js("return document.querySelector('[data-testid=dictation-settings] kbd')?.textContent")
    s.check(row == "Ctrl+Shift+Space", f"it shows the key: {row!r}")
    s.check(bool(s.js("return document.querySelector('[data-testid=dictation-on]')")), "dictation is off until turned on")
    themed(s, "settings-dictation-off")

    # Record from the virtual mic only.
    sources = s.invoke("list_sources")
    mic = next((x for x in sources if "Manent-smoke-mic" in x["name"]), None)
    s.check(mic is not None, f"the virtual mic is listed: {[x['name'] for x in sources]}")
    s.invoke("set_recording_sources", {"mic": mic["id"], "system": None})

    # Turn it on; GNOME asks once (the dialog's Add button: Tab x3, Enter).
    def accept():
        time.sleep(3)
        shot(str(OUT / "gnome-dialog.png"))
        for _ in range(3):
            kb.tap(KEY["tab"])
            time.sleep(0.4)
        kb.tap(KEY["enter"])

    threading.Thread(target=accept, daemon=True).start()
    s.js("document.querySelector('[data-testid=dictation-on]').click()")
    on = s.wait_for("return document.querySelector('[data-testid=dictation-off]')", 30)
    msg = s.js("return document.querySelector('[data-testid=shortcuts-message]')?.textContent")
    s.check(bool(on), f"dictation turned on (message: {msg})")
    status = s.invoke("get_shortcuts")
    s.check(status["dictation"] and not status["enabled"], f"only dictation is bound: {status}")
    s.check([x["id"] for x in status["shortcuts"]] == ["dictate"], f"the desktop bound the key: {status['shortcuts']}")
    themed(s, "settings-dictation-on")

    # No engine yet: dictation says so.
    n = log_len()
    hold(1.0)
    s.check(bool(wait_log(n, "notification: Dictation needs the transcription engine", 5)),
            f"without the engine a notification says so: {log_lines(n)}")
    for name in ("engine", "models"):
        (DATA / name).symlink_to(ENGINE_DATA / name)

    # A tap does nothing.
    n = log_len()
    hold(0.1)
    time.sleep(1.5)
    lines = log_lines(n)
    s.check(any("dictate Pressed" in l for l in lines) and not any("transcribed" in l or "notification" in l for l in lines),
            f"a tap does nothing: {lines}")

    # Dutch and English.
    for lang, (wav, words) in SPEECH.items():
        n = log_len()
        subprocess.run(["wl-copy", "previous"], check=True)
        t0 = time.monotonic()
        hold(0.5, wav)
        released = time.monotonic()
        ready = wait_log(n, "notification: Ready to paste", 60)
        took = time.monotonic() - released
        text = clipboard()
        s.check(bool(ready) and all(w in text.lower() for w in words),
                f"{lang}: {time.monotonic() - t0 - took:.1f} s held, ready {took:.2f} s after release: {text!r}")
        timing = wait_log(n, "transcribed in", 1)
        print(f"     {timing}")

    # Silence: nothing heard.
    n = log_len()
    hold(1.5)
    s.check(bool(wait_log(n, "notification: Nothing was heard", 20)), f"silence: nothing heard: {log_lines(n)}")

    meetings, files, runtime = stored()
    s.check(meetings == 0 and not files and not runtime,
            f"nothing stored: {meetings} meetings, files {files}, runtime {runtime}")

    # During a meeting recording the key only explains itself.
    started = s.invoke("start_recording")
    s.check("meeting_id" in started, f"a meeting recording runs: {started}")
    n = log_len()
    hold(1.5, SPEECH["en"][0])
    time.sleep(2)
    lines = log_lines(n)
    s.check(any("isn't available while recording" in l for l in lines) and not any("transcribed" in l for l in lines),
            f"blocked during a recording: {lines}")
    s.invoke("stop_recording")
    s.invoke("delete_meeting", {"id": started["meeting_id"]})
    failures += s.failures
    s.close()

    # After a restart dictation is still on, without asking again.
    s = Session(APP)
    open_settings(s)
    on = s.wait_for("return document.querySelector('[data-testid=dictation-off]')", 20)
    s.check(bool(on), "dictation is still on after a restart")
    n = log_len()
    hold(0.5, SPEECH["en"][0])
    ready = wait_log(n, "notification: Ready to paste", 60)
    s.check(bool(ready) and "moldova" in clipboard().lower(), f"and works: {clipboard()!r}")

    # Auto-paste: refusing the desktop's dialog turns it off again, with a reason.
    s.wait_for("return document.querySelector('[data-testid=autopaste-on]')", 10)
    themed(s, "settings-autopaste-off")

    def refuse():
        time.sleep(3)
        kb.tap(KEY["esc"])

    threading.Thread(target=refuse, daemon=True).start()
    s.js("document.querySelector('[data-testid=autopaste-on]').click()")
    msg = s.wait_for("return document.querySelector('[data-testid=autopaste-message]')?.textContent", 30)
    s.check(bool(msg) and bool(s.js("return document.querySelector('[data-testid=autopaste-on]')")),
            f"refusing the dialog leaves it off: {msg!r}")
    s.check(setting("dictation.autopaste") in (None, "false"), f"and stored off: {setting('dictation.autopaste')}")

    threading.Thread(target=accept_remote_desktop, daemon=True).start()
    s.js("document.querySelector('[data-testid=autopaste-on]').click()")
    on = s.wait_for("return document.querySelector('[data-testid=autopaste-off]')", 30)
    msg = s.js("return document.querySelector('[data-testid=autopaste-message]')?.textContent")
    s.check(bool(on), f"accepting turns it on (message: {msg})")
    s.check(bool(setting("dictation.paste_token")) and setting("dictation.paste_token") != "null",
            "the restore token is stored")
    themed(s, "settings-autopaste-on")

    gtk_out = OUT / "gtk-target.txt"
    gtk = ["python3", str(ROOT / "spikes" / "dictation" / "target.py"), str(gtk_out)]
    # Ctrl+Shift stay down after Space: typing waits for them, or "a" would be Ctrl+Shift+A.
    text = paste_into(s, "typed into a GTK text field", gtk, gtk_out, "moldova", "Typed", linger=1.5)
    s.check(clipboard() == "earlier clipboard é", f"and the clipboard is untouched: {clipboard()!r}")

    # Terminals get typed text too: no paste key involved.
    term_out = OUT / "terminal.txt"
    paste_into(s, "typed into a terminal (alacritty)",
               ["alacritty", "-e", "sh", "-c", f"cat > {term_out}"], term_out, text or "moldova",
               "Typed", enter=True)

    # A layout without latin letters: those are pasted, the rest is typed.
    s.js("const k = document.querySelector('[data-testid=paste-key]'); k.value = 'ctrl-shift-v';"
         "k.dispatchEvent(new Event('change', {bubbles: true}))")
    time.sleep(1)
    s.check(setting("dictation.paste_key") == '"ctrl-shift-v"', f"the paste key is stored: {setting('dictation.paste_key')}")
    s.js("const k = document.querySelector('[data-testid=paste-key]'); k.value = 'ctrl-v';"
         "k.dispatchEvent(new Event('change', {bubbles: true}))")
    time.sleep(1)
    layout("ru")
    paste_into(s, "with a Russian layout the letters are pasted", gtk, gtk_out, text or "moldova",
               "Restored")
    time.sleep(0.5)
    s.check(clipboard() == "earlier clipboard é", f"and the earlier clipboard is back: {clipboard()!r}")
    layout("us")
    failures += s.failures
    s.close()

    # After a restart auto-paste is on again without a dialog.
    s = Session(APP)
    open_settings(s)
    on = s.wait_for("return document.querySelector('[data-testid=autopaste-off]')", 20)
    s.check(bool(on), "auto-paste is still on after a restart, without asking")
    paste_into(s, "and types", gtk, gtk_out, text or "moldova", "Typed")
    s.js("document.querySelector('[data-testid=autopaste-off]').click()")
    s.check(bool(s.wait_for("return document.querySelector('[data-testid=autopaste-on]')", 10)), "auto-paste turned off")
    s.check(setting("dictation.paste_token") in (None, "null"), f"and the token is forgotten: {setting('dictation.paste_token')}")

    # Turning it off lets go of the key.
    s.js("document.querySelector('[data-testid=dictation-off]').click()")
    s.check(bool(s.wait_for("return document.querySelector('[data-testid=dictation-on]')", 10)), "turned off")
    n = log_len()
    hold(1.0)
    time.sleep(1)
    s.check(not log_lines(n), f"the key does nothing when off: {log_lines(n)}")
    failures += s.failures
    s.close()
finally:
    driver.terminate()
    loopback.terminate()

print("FAILED" if failures else "all checks passed", f"(screenshots in {OUT})")
sys.exit(1 if failures else 0)
