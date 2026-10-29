"""Settings > Sources smoke check on the real app (Linux/PipeWire, needs audio hardware).

Usage: python3 scripts/smoke-settings.py <app binary> <data dir> <screenshot dir>

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (an empty temp folder).
Briefly switches the OS default microphone with pactl to check that "System default"
follows it, and always restores the original default.
"""
import json
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])


def pactl(*args):
    return subprocess.run(["pactl", *args], capture_output=True, text=True, check=True).stdout


def open_settings(s):
    s.wait_for("return document.querySelectorAll('nav[aria-label=Main] button').length === 3 && true")
    s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Settings')).click()")
    return s.wait_for("return document.querySelector('[data-testid=mic-select]') && document.querySelector('[data-testid=mic-select]').options.length > 0 && true", 10)


def options(s, testid):
    return s.js(f"return [...document.querySelector('[data-testid={testid}]').options].map(o => [o.value, o.textContent])")


def select(s, testid, value):
    s.js(f"const el = document.querySelector('[data-testid={testid}]');"
         "Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, arguments[0]);"
         "el.dispatchEvent(new Event('change', {bubbles: true}));", value)


def meters(s):
    return s.js("return [...document.querySelectorAll('[data-testid=level-meter]')].map(m => +m.getAttribute('aria-valuenow'))")


def setting(key):
    con = sqlite3.connect(DATA / "veritas.db")
    row = con.execute("SELECT value FROM settings WHERE key = ?", (key,)).fetchone()
    con.close()
    return json.loads(row[0]) if row else None


s = Session(APP)
s.check(bool(open_settings(s)), "Settings > Sources shows the pickers")
mic_opts, sys_opts = options(s, "mic-select"), options(s, "system-select")
print("     mic options:", [t for _, t in mic_opts])
print("     system options:", [t for _, t in sys_opts])
s.check(mic_opts[0][1] == "System default microphone" and len(mic_opts) >= 2, "mic picker lists the system default first plus real mics")
s.check(sys_opts[0][1] == "System default output" and sys_opts[-1][1].startswith("Don"), "system picker lists outputs and 'Don't record'")

samples = []
for _ in range(8):
    samples.append(meters(s))
    time.sleep(0.25)
mic_levels = [m[0] for m in samples]
s.check(len(set(mic_levels)) > 2 and max(mic_levels) > 0, f"mic test meter is live: {mic_levels}")
for theme in ("light", "dark"):
    s.js("document.documentElement.classList.toggle('dark', arguments[0] === 'dark')", theme)
    time.sleep(0.6)
    s.screenshot(OUT / f"settings-sources-{theme}.png")
s.js("document.documentElement.classList.toggle('dark', matchMedia('(prefers-color-scheme: dark)').matches)")

# "System default" follows OS default changes (monitor stream moves to the new default).
original = pactl("get-default-source").strip()
mic_names = [l.split("\t")[1] for l in pactl("list", "short", "sources").splitlines() if ".monitor" not in l]
other = next(n for n in mic_names if n != original)


def app_stream_sources():
    """Source names our app's capture streams are connected to."""
    sources = {l.split("\t")[0]: l.split("\t")[1] for l in pactl("list", "short", "sources").splitlines()}
    found = []
    for block in pactl("list", "source-outputs").split("\n\n"):
        if re.search(r'(application|node)\.name = "(veritas|cpal)', block, re.I) or "veritas" in block.lower():
            m = re.search(r"Source: (\d+)", block)
            if m:
                found.append(sources.get(m.group(1), m.group(1)))
    return found


try:
    before = app_stream_sources()
    print(f"     default {original}; app streams on {before}")
    pactl("set-default-source", other)
    time.sleep(1.5)
    after = app_stream_sources()
    print(f"     default -> {other}; app streams on {after}")
    s.check(original in before and other in after and original not in after,
            "System default microphone follows the OS default (stream moved)")
finally:
    pactl("set-default-source", original)
    time.sleep(1.0)
    s.check(pactl("get-default-source").strip() == original, f"OS default restored to {original}")
level_back = []
for _ in range(6):
    level_back.append(meters(s)[0])
    time.sleep(0.25)
s.check(len(set(level_back)) > 1, f"meter follows back to the original default and is live: {level_back}")

# Choose a specific mic and no system audio; persisted in the settings table.
specific = next(v for v, t in mic_opts if not t.startswith("System default"))
select(s, "mic-select", specific)
time.sleep(0.4)
select(s, "system-select", "")
time.sleep(0.6)
stored = setting("recording.sources")
s.check(stored == [specific], f"choice stored in settings: {stored}")
s.close()

# Restart: the choice is shown again and used by the next recording.
s = Session(APP)
open_settings(s)
values = s.js("return [document.querySelector('[data-testid=mic-select]').value, document.querySelector('[data-testid=system-select]').value]")
s.check(values == [specific, ""], f"choice shown after restart: {values}")
started = s.invoke("start_recording")
used = [x["id"] for x in started.get("sources", [])] if isinstance(started, dict) else started
s.check(used == [specific], f"next recording uses the configured sources: {used}")
s.invoke("stop_recording")
s.close()

print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
