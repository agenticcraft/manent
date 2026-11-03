"""The rename from Veritas to Manent on the real app (VRT-44): an old install is carried over.

Usage: python3 scripts/smoke-rename.py <app binary> <source data dir> <xdg data home> <screenshot dir> [<server> <user> <password>]

Builds an install as Veritas left it, from a data folder with meetings (and the engine): at
<xdg data home>/nl.veritas.app, the database as veritas.db with its paths pointing there.
Large files are hard links, the database a copy, so the source isn't changed. tauri-driver
must run with XDG_DATA_HOME=<xdg data home> and without MANENT_DATA_DIR/VERITAS_DATA_DIR, so
the app uses its real (here: test) OS data folder. With a Nextcloud test account the old
install is also connected (app password under the old keychain service); start
tauri-driver with MANENT_NO_BROWSER=1 then.

Checks: the folder moved to ai.agenticcraft.manent with manent.db; meetings still open, play
and search; paths in the database point at the new folder; the window says Manent; with sync,
the keychain entry moved to the new service, the server folder stays "Veritas" and a round works.
"""
import shutil
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, SOURCE, XDG, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
SYNC = sys.argv[5:8] if len(sys.argv) > 7 else None
OLD, NEW = XDG / "nl.veritas.app", XDG / "ai.agenticcraft.manent"
OUT.mkdir(parents=True, exist_ok=True)
ROOT = Path(__file__).resolve().parent.parent

# 1. An install as Veritas left it.
shutil.rmtree(OLD, ignore_errors=True)
shutil.rmtree(NEW, ignore_errors=True)
db_name = "manent.db" if (SOURCE / "manent.db").exists() else "veritas.db"
# The database (with its WAL, where recent writes still are) is copied; the rest hard-linked.
shutil.copytree(SOURCE, OLD, copy_function=lambda s, d: shutil.copy2(s, d) if Path(s).name.startswith(db_name)
                else Path(d).hardlink_to(s), ignore=shutil.ignore_patterns("tmp"))
con = sqlite3.connect(OLD / db_name)
con.execute("PRAGMA wal_checkpoint(TRUNCATE)")
con.close()
if db_name != "veritas.db":
    for suffix in ["", "-wal", "-shm"]:
        if (OLD / f"{db_name}{suffix}").exists():
            (OLD / f"{db_name}{suffix}").rename(OLD / f"veritas.db{suffix}")
con = sqlite3.connect(OLD / "veritas.db")
src, old = str(SOURCE).rstrip("/"), str(OLD)
for table, col in [("meetings", "dir"), ("tracks", "path")]:
    con.execute(f"UPDATE {table} SET {col} = ? || substr({col}, length(?) + 1) WHERE {col} LIKE ? || '%'", (old, src, src))
con.execute("DELETE FROM settings WHERE key LIKE 'sync.%'")
con.execute("DELETE FROM sync_meetings")
con.execute("DELETE FROM sync_inbox")
meetings = con.execute("SELECT COUNT(*) FROM meetings").fetchone()[0]
con.commit()
con.close()
print(f"     old install: {meetings} meetings in {OLD}")

account = None
if SYNC:
    server, user, password = SYNC
    import json

    import requests
    flow = requests.post(f"{server}/index.php/login/v2").json()
    subprocess.run([sys.executable, "-I", str(ROOT / "spikes/nextcloud/nc_grant.py"), flow["login"], user, password],
                   check=True, capture_output=True)
    got = requests.post(flow["poll"]["endpoint"], data={"token": flow["poll"]["token"]}).json()
    account = f"{got['loginName']}@{got['server']}"
    subprocess.run(["secret-tool", "store", "--label=Veritas test", "service", "veritas-nextcloud", "username", account],
                   input=got["appPassword"], text=True, check=True)
    con = sqlite3.connect(OLD / "veritas.db")
    for k, v in [("sync.server", got["server"]), ("sync.login", got["loginName"])]:
        con.execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)", (k, json.dumps(v)))
    con.commit()
    con.close()
    requests.request("MKCOL", f"{server}/remote.php/dav/files/{user}/Veritas", auth=(user, password))


def secrets(service):
    out = subprocess.run(["secret-tool", "search", "--all", "--unlock", "service", service],
                         capture_output=True, text=True).stdout
    return [line for line in out.splitlines() if line.startswith("secret = ")]


# 2. Start Manent.
s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 30)
s.check(NEW.joinpath("manent.db").is_file() and not OLD.exists(), f"data moved to {NEW} as manent.db")
con = sqlite3.connect(NEW / "manent.db")
dirs = [d for (d,) in con.execute("SELECT dir FROM meetings")]
paths = [p for (p,) in con.execute("SELECT path FROM tracks")]
con.close()
s.check(len(dirs) == meetings and all(d.startswith(str(NEW)) for d in dirs + paths),
        f"database paths point at the new folder ({len(dirs)} meetings, {len(paths)} tracks)")
title = s.js("return document.title")
s.check(title == "Manent", f"window title {title!r}")
mid = max(s.invoke("list_meetings"), key=lambda m: m["duration_ms"])["id"]
s.js(f"document.querySelector(\"[data-testid=meeting-row][data-meeting-id='{mid}'] [data-testid=open-meeting]\").click()")
dur = s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 20)
words = s.wait_for("return document.querySelectorAll('[data-testid=word]').length", 20)
s.check(bool(dur) and bool(words), f"a meeting opens and plays after the move ({words} words, {dur} s)")
s.screenshot(OUT / "rename-meeting-dark.png")
s.js("document.querySelector('[data-testid=back]').click()")
some_word = next((w["text"] for seg in s.invoke("live_transcript", {"meetingId": mid}) for w in seg["words"]
                  if len(w["text"]) > 6), None)
hits = s.invoke("search_transcripts", {"query": some_word.strip(".,?!")}) if some_word else []
s.check(bool(hits), f"search still finds '{some_word}' ({len(hits)} hits)")
mds = [Path(d) / "transcript.md" for d in dirs if (Path(d) / "transcript.md").exists()]
end = time.time() + 20
while time.time() < end and not all(m.read_text().startswith("---\nmanent: 1") for m in mds):
    time.sleep(0.5)
s.check(bool(mds) and all(m.read_text().startswith("---\nmanent: 1") for m in mds),
        f"transcript.md files rewritten with the new header ({len(mds)})")

if SYNC:
    status = None
    end = time.time() + 60
    while time.time() < end:
        status = s.invoke("sync_status")
        if status["state"] == "idle" and status["last_sync"]:
            break
        time.sleep(1)
    s.check(status["state"] == "idle" and status["folder"] == "Veritas",
            f"the Nextcloud connection survives: {status['state']}, folder {status['folder']!r} ({status['error']})")
    s.check(len(secrets("manent-nextcloud")) == 1 and not secrets("veritas-nextcloud"),
            "the app password moved to the new keychain service")
    s.invoke("sync_disconnect")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
