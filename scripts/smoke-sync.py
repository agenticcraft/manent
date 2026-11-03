"""Nextcloud sync on the real app (VRT-39) against a local Nextcloud, never a real account.

Usage: python3 scripts/smoke-sync.py <app binary> <data dir> <screenshot dir> <server> <user> <password> <speech.wav>

Needs the engine (the phone recording is transcribed) and a local Nextcloud with a test user
(see spikes/nextcloud/e2e.py for the container). tauri-driver must run with
MANENT_DATA_DIR=<data dir> and MANENT_NO_BROWSER=1 (the script approves the login itself,
with spikes/nextcloud/nc_grant.py, as the user would in the browser). Empties the test user's
Manent folder first.

Checks: connecting in Settings (app password in the keychain, not in the database); a phone
recording with meta.json in the inbox is imported with its title, transcribed, moved on the
server into "<date time> <title>/" with transcript.md next to it; naming a speaker updates the
transcript on the server; "all meetings" adds desktop meetings (mix.opus + transcript.md); a
folder deleted on the server is left alone; deleting a meeting can remove its folder; disconnect
revokes the app password. With NC_CONTAINER=<docker container>, the server is stopped and
started once to check that sync reports the error and recovers.
"""
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

import requests

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SERVER, USER, PASSWORD, SPEECH = sys.argv[1:8]
DATA, OUT = Path(DATA), Path(OUT)
OUT.mkdir(parents=True, exist_ok=True)
ROOT = Path(__file__).resolve().parent.parent
DAV = f"{SERVER}/remote.php/dav/files/{USER}"
auth = (USER, PASSWORD)


def ls(path):
    """Names below a folder on the server (Depth 1), or None if it doesn't exist."""
    r = requests.request("PROPFIND", f"{DAV}/{path}", auth=auth, headers={"Depth": "1"})
    if r.status_code == 404:
        return None
    hrefs = [e.text for e in ET.fromstring(r.content).iter("{DAV:}href")]
    base = f"/remote.php/dav/files/{USER}/{path}".rstrip("/") + "/"
    return sorted(requests.utils.unquote(h)[len(base):].rstrip("/") for h in hrefs
                  if requests.utils.unquote(h).rstrip("/") + "/" != base)


def wait(cond, timeout, step=1.0):
    end = time.time() + timeout
    while time.time() < end:
        v = cond()
        if v:
            return v
        time.sleep(step)
    return cond()


def _up():
    try:
        return requests.get(f"{SERVER}/status.php", timeout=3).ok
    except requests.RequestException:
        return False


def secret():
    out = subprocess.run(["secret-tool", "search", "--all", "--unlock", "service", "manent-nextcloud"],
                         capture_output=True, text=True).stdout
    return next((line.split("= ", 1)[1] for line in out.splitlines() if line.startswith("secret = ")), None)


def db(query, *args):
    con = sqlite3.connect(DATA / "manent.db")
    rows = con.execute(query, args).fetchall()
    con.close()
    return rows


# A clean start on the server.
requests.request("DELETE", f"{DAV}/Manent", auth=auth)

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
s.invoke("set_rolling_interval", {"seconds": 45})
s.invoke("sync_disconnect")  # from a clean state (harmless when not connected)
s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Settings')).click()")
s.wait_for("return document.querySelector('[data-testid=sync-server]')", 10)

s.check(s.invoke("sync_status")["mode"] == "phone", "a new connection starts with phone recordings only")

# 1. Connect.
s.js("const i = document.querySelector('[data-testid=sync-server]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
     "i.dispatchEvent(new Event('input', {bubbles: true}))", SERVER)
s.js("document.querySelector('[data-testid=sync-connect]').click()")
s.wait_for("return document.querySelector('[data-testid=sync-connecting]') && true", 10)
url = s.invoke("sync_status")["login_url"]
s.screenshot(OUT / "sync-connecting-dark.png")
grant = subprocess.run([sys.executable, "-I", str(ROOT / "spikes/nextcloud/nc_grant.py"), url, USER, PASSWORD],
                       capture_output=True, text=True)
connected = wait(lambda: s.invoke("sync_status")["state"] in ("idle", "syncing") and s.invoke("sync_status"), 30)
s.check(bool(connected) and connected["login"] == USER, f"connected via Login Flow v2 ({grant.stdout.strip()}): {connected}")
app_password = secret()
stored = [v for (k, v) in db("SELECT key, value FROM settings WHERE key LIKE 'sync.%'")]
s.check(bool(app_password) and not any(app_password in v for v in stored),
        f"app password in the keychain, not in the database (settings: {stored})")
s.check(wait(lambda: ls("Manent") == ["Inbox"], 30), f"the Manent/Inbox folder is made ({ls('Manent')})")

# 2. A phone recording in the inbox.
rec = OUT / "phone-recording.opus"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", "25", "-t", "60", "-i", SPEECH, "-ac", "1",
                "-c:a", "libopus", "-b:a", "32k", str(rec)], check=True)
started = int(time.time() * 1000) - 3_600_000
requests.put(f"{DAV}/Manent/Inbox/phone-0912.opus", data=rec.read_bytes(), auth=auth).raise_for_status()
requests.put(f"{DAV}/Manent/Inbox/phone-0912.meta.json", auth=auth,
             data=json.dumps({"title": "Smoke phone meeting", "started_at": started, "device": "test"})).raise_for_status()
s.invoke("sync_now")
meeting = wait(lambda: next((m for m in s.invoke("list_meetings") if m["title"] == "Smoke phone meeting"), None), 60)
s.check(bool(meeting) and abs(meeting["started_at"] - started) < 1000,
        f"imported with the title and start time from meta.json ({meeting and meeting['id']})")
folder = wait(lambda: next((n for n in (ls("Manent") or []) if n.endswith("Smoke phone meeting")), None), 300, 2)
files = ls(f"Manent/{folder}") if folder else None
s.check(bool(folder) and re.fullmatch(r"\d{4}-\d\d-\d\d \d\d\.\d\d Smoke phone meeting", folder or "")
        and files == ["phone-0912.meta.json", "phone-0912.opus", "transcript.md"] and ls("Manent/Inbox") == [],
        f"moved into {folder!r} with its transcript ({files}); inbox {ls('Manent/Inbox')}")
md = requests.get(f"{DAV}/Manent/{folder}/transcript.md", auth=auth).text if folder else ""
s.check(md.startswith("---\nmanent: 1\n") and "Smoke phone meeting" in md and "**" in md,
        f"transcript.md on the server ({len(md)} chars)")
s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Library')).click()")
line = s.wait_for("const l = document.querySelector('[data-testid=sync-line]'); return l && l.textContent", 10)
s.check(bool(line) and "Nextcloud" in line, f"the Library shows the sync state: {line!r}")
s.screenshot(OUT / "sync-library-dark.png")

# 2b. The server goes away and comes back (NC_CONTAINER names its Docker container).
container = os.environ.get("NC_CONTAINER")
if container:
    subprocess.run(["docker", "stop", container], capture_output=True, check=True)
    s.invoke("sync_now")
    down = wait(lambda: s.invoke("sync_status")["state"] == "error" and s.invoke("sync_status")["error"], 60)
    line = s.wait_for("const l = document.querySelector('[data-testid=sync-line][role=alert]'); return l && l.textContent", 10)
    s.check(bool(down) and bool(line), f"offline: the Library says sync failed ({line!r})")
    subprocess.run(["docker", "start", container], capture_output=True, check=True)
    wait(_up, 120, 2)
    s.invoke("sync_now")
    s.check(bool(wait(lambda: s.invoke("sync_status")["state"] == "idle", 60)), "back online: sync recovers by itself")

# 3. Naming a speaker updates the transcript on the server.
voices = s.invoke("meeting_voices", {"meetingId": meeting["id"]}) if meeting else []
if voices:
    s.invoke("name_speaker", {"meetingId": meeting["id"], "label": voices[0]["label"], "name": "Smoke Sync Person"})
    s.check(bool(wait(lambda: "Smoke Sync Person" in requests.get(f"{DAV}/Manent/{folder}/transcript.md", auth=auth).text, 30)),
            "naming a speaker updates transcript.md on the server")

# 4. All meetings: desktop meetings get a folder with mix.opus and transcript.md.
s.invoke("sync_set_mode", {"mode": "all"})
done = [m for m in s.invoke("list_meetings") if m["status"] == "done"]
synced = wait(lambda: len([m for m in s.invoke("list_meetings") if m["synced"]]) >= len(done) and True, 300, 3)
desktop = db("SELECT meeting_id, remote_dir, mix_uploaded, transcript_hash FROM sync_meetings WHERE origin = 'desktop'")
sample = next((d for d in desktop if d[2] and d[3]), None)  # with a playback file and a transcript
sample_files = ls(sample[1]) if sample else None
s.check(bool(synced) and sample is not None and sample_files == ["mix.opus", "transcript.md"],
        f"all meetings: {len(desktop)} desktop meetings synced; e.g. {sample and sample[1]}: {sample_files}")

# 5. A folder deleted on the server is left alone.
if sample:
    requests.request("DELETE", f"{DAV}/{sample[1]}", auth=auth)
    # A new title, so transcript.md really changes and an upload is tried.
    s.invoke("rename_meeting", {"id": sample[0], "title": f"Smoke renamed after remote delete {int(time.time())}"})
    gone = wait(lambda: db("SELECT gone FROM sync_meetings WHERE meeting_id = ?", sample[0])[0][0] == 1, 60)
    s.check(bool(gone) and ls(sample[1]) is None, "a folder deleted on the server is not uploaded again")

# 6. Deleting the phone meeting with "also from Nextcloud".
s.invoke("sync_set_mode", {"mode": "phone"})
if meeting:
    s.invoke("delete_meeting", {"id": meeting["id"], "alsoRemote": True})
    s.check(ls(f"Manent/{folder}") is None, "deleting the meeting removed its folder on the server")

# 7. Settings, then disconnect.
s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Settings')).click()")
s.wait_for("return document.querySelector('[data-testid=sync-account]')", 10)
s.js("document.querySelector('[data-testid=sync-settings]').scrollIntoView({block: 'center'})")
time.sleep(0.4)
s.screenshot(OUT / "sync-settings-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.4)
s.screenshot(OUT / "sync-settings-light.png")
s.js("document.documentElement.classList.add('dark')")
s.js("document.querySelector('[data-testid=sync-disconnect]').click()")
s.wait_for("return document.querySelector('[data-testid=sync-disconnect-confirm]')", 5)
s.js("document.querySelector('[data-testid=sync-disconnect-confirm]').click()")
off = wait(lambda: s.invoke("sync_status")["state"] == "off", 20)
code = requests.request("PROPFIND", f"{DAV}/Manent", auth=(USER, app_password or "x")).status_code
s.check(bool(off) and secret() is None and code == 401,
        f"disconnect revokes the app password (server answers {code}) and empties the keychain")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
