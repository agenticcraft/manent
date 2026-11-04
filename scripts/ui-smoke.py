"""UI smoke check of the real Tauri app through tauri-driver (WebDriver), no extra deps.

Usage: python3 scripts/ui-smoke.py <app binary> <screenshot dir> [--driver http://127.0.0.1:4444]

Starts the app, checks the shell (sidebar items, page titles, IPC round trip on Settings),
toggles the theme and saves screenshots of each page in light and dark.
Requires: tauri-driver running and WebKitWebDriver installed (Linux).
"""
import base64
import json
import sys
import time
import urllib.request
from pathlib import Path

args = sys.argv[1:]
APP, OUT = args[0], Path(args[1])
DRIVER = args[args.index("--driver") + 1] if "--driver" in args else "http://127.0.0.1:4444"
OUT.mkdir(parents=True, exist_ok=True)


def wd(method, path, body=None):
    req = urllib.request.Request(DRIVER + path, data=json.dumps(body).encode() if body is not None else None,
                                 method=method, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read() or b"{}").get("value")
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path} -> {e.code}: {e.read().decode()[:300]}") from None


session = wd("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {"application": APP}}}})
sid = session["sessionId"]
S = f"/session/{sid}"
failures = []


def check(cond, msg):
    print(("ok   " if cond else "FAIL ") + msg)
    if not cond:
        failures.append(msg)


def js(script, *a):
    return wd("POST", f"{S}/execute/sync", {"script": script, "args": list(a)})


def js_async(script, *a):
    return wd("POST", f"{S}/execute/async", {"script": script, "args": list(a)})


def shot(name):
    (OUT / f"{name}.png").write_bytes(base64.b64decode(wd("GET", f"{S}/screenshot")))


try:
    for _ in range(30):  # wait until the webview has rendered the shell
        try:
            if js("return document.querySelectorAll('nav[aria-label=Main] button').length") == 4:
                break
        except RuntimeError:
            pass
        time.sleep(0.5)
    nav = js("return [...document.querySelectorAll('nav[aria-label=Main] button')].map(b => b.textContent.trim())")
    check(nav == ["Record", "Library", "Speakers", "Settings"], f"sidebar items {nav}")
    font = js("return getComputedStyle(document.body).fontFamily")
    check("Inter" in font, f"body font is Inter ({font})")
    system_dark = js("return matchMedia('(prefers-color-scheme: dark)').matches")
    has_dark = js("return document.documentElement.classList.contains('dark')")
    check(system_dark == has_dark, f"theme follows OS (prefers dark={system_dark}, .dark={has_dark})")

    sources = js_async(
        "const done = arguments[arguments.length - 1];"
        "window.__TAURI_INTERNALS__.invoke('list_sources').then(done, e => done({error: String(e)}));"
    )
    ok = isinstance(sources, list)
    kinds = sorted({s["kind"] for s in sources}) if ok else sources
    check(ok and kinds == ["loopback", "mic"], f"IPC list_sources returns mic and loopback sources ({len(sources) if ok else sources})")
    if ok:
        for s in sources:
            print(f"     source: {s['kind']:8} default={s['is_default']!s:5} {s['name']}")

    # Recording round trip: start (system defaults), level events, pause/resume, stop.
    if ok:
        ids = [s["id"] for s in sources if s["is_default"]]
        js_async(
            "const done = arguments[arguments.length - 1]; const T = window.__TAURI_INTERNALS__;"
            "window.__levels = [];"
            "T.invoke('plugin:event|listen', {event: 'audio:level', target: {kind: 'Any'},"
            " handler: T.transformCallback(e => window.__levels.push(e.payload))}).then(() => done(true), e => done(String(e)));"
        )
        invoke = ("const done = arguments[arguments.length - 1];"
                  "window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1] || {}).then(done, e => done({error: String(e)}));")
        started = js_async(invoke, "start_recording")
        check(isinstance(started, dict) and "meeting_id" in started, f"start_recording -> {started}")
        time.sleep(1.5)
        js_async(invoke, "pause_recording")
        time.sleep(1.0)
        js_async(invoke, "resume_recording")
        time.sleep(1.5)
        stopped = js_async(invoke, "stop_recording")
        summary = stopped.get("tracks") if isinstance(stopped, dict) else stopped
        levels = js("return window.__levels")
        rate = len(levels) / len(ids) / 4.0 if levels else 0
        check(isinstance(summary, list) and len(summary) == 2, f"stop_recording returned {len(summary) if isinstance(summary, list) else summary} tracks")
        check(any(l["paused"] for l in levels) and 15 <= rate <= 25, f"audio:level events in the UI: {len(levels)} ({rate:.1f}/s per source, paused seen: {any(l['paused'] for l in levels)})")
        if isinstance(summary, list):
            for t in summary:
                exists = Path(t["file"]).is_file()
                check(exists and 2.5 < t["seconds"] < 3.5 and t["dropped_samples"] == 0,
                      f"{t['kind']} track {Path(t['file']).name}: {t['seconds']:.2f}s, dropped {t['dropped_samples']}, file exists {exists}")

    for theme in ("light", "dark"):
        js("document.documentElement.classList.toggle('dark', arguments[0] === 'dark')", theme)
        for i, page in enumerate(["Library", "Speakers", "Settings"]):
            js("document.querySelectorAll('nav[aria-label=Main] ul button')[arguments[0]].click()", i)
            time.sleep(0.6)
            title = js("return document.querySelector('main h1')?.textContent")
            current = js("return document.querySelector('nav [aria-current=page]')?.textContent.trim()")
            if theme == "light":
                check(title == page and current == page, f"{page}: heading '{title}', active nav '{current}'")
            if page == "Settings":
                info = js("return document.querySelector('[data-testid=app-info]')?.textContent")
                if theme == "light":
                    check(bool(info) and info.startswith("Manent ") and "linux" in info, f"IPC app_info shown: '{info}'")
            bg = js("return getComputedStyle(document.body).backgroundColor")
            print(f"     {theme} {page}: body background {bg}")
            shot(f"{page.lower()}-{theme}")
finally:
    wd("DELETE", S)

print(f"\n{'PASS' if not failures else 'FAILED: ' + '; '.join(failures)}; screenshots in {OUT}")
sys.exit(1 if failures else 0)
