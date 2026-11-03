"""Library smoke check on the real app with seeded data.

Usage: python3 scripts/smoke-library.py <app binary> <data dir> <screenshot dir>

tauri-driver must run with MANENT_DATA_DIR=<data dir> (an empty temp folder), so the app
never touches real data. The script starts the app once (creates + migrates the DB), seeds
meetings with speakers and audio folders, restarts the app and checks the Library.
"""
import re
import sqlite3
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
DB = DATA / "manent.db"

# 1. First start creates the database; nothing to check yet except an empty library.
s = Session(APP)
s.wait_for("return document.querySelector('main h1')")
empty = s.wait_for("return document.body.textContent.includes('No meetings yet')")
s.check(bool(empty), "empty library shows the empty state")
s.screenshot(OUT / "library-empty.png")
failures = list(s.failures)
s.close()

# 2. Seed: three meetings (out of order), speakers on two of them, audio folders with files.
con = sqlite3.connect(DB)
con.execute("PRAGMA foreign_keys = ON")
meetings = [
    ("Weekly planning", 1_790_000_000_000, 3_725_000, "done"),
    ("Klantgesprek Acme", 1_793_000_000_000, 1_830_000, "done"),
    # 'failed': a status startup leaves alone ('processing' is re-queued since VRT-19 and
    # 'recording' recovered since VRT-41).
    ("", 1_791_500_000_000, 42_000, "failed"),
]
ids = {}
for title, started, dur, status in meetings:
    d = DATA / "meetings" / f"seed-{started}"
    d.mkdir(parents=True, exist_ok=True)
    (d / "mic.flac").write_bytes(b"fake")
    cur = con.execute(
        "INSERT INTO meetings (title, started_at, duration_ms, status, dir) VALUES (?, ?, ?, ?, ?)",
        (title, started, dur, status, str(d)),
    )
    ids[title] = (cur.lastrowid, d)
spk = {name: con.execute("INSERT INTO speakers (name, color) VALUES (?, ?)", (name, c)).lastrowid
       for name, c in [("Bram Jansen", 1), ("Anna de Vries", 3), ("Chris", 5)]}
klant, _ = ids["Klantgesprek Acme"]
weekly, weekly_dir = ids["Weekly planning"]
for i, name in enumerate(["Anna de Vries", "Bram Jansen", "Anna de Vries"]):
    con.execute("INSERT INTO segments (meeting_id, pass, speaker_id, start_ms, end_ms, text) VALUES (?, 'final', ?, ?, ?, 'x')",
                (klant, spk[name], i * 1000, i * 1000 + 900))
con.execute("INSERT INTO segments (meeting_id, pass, speaker_id, start_ms, end_ms, text) VALUES (?, 'final', ?, 0, 1, 'y')",
            (weekly, spk["Chris"]))
con.commit()
con.close()

# 3. Restart and check the Library.
s = Session(APP)
s.failures = failures
rows = s.wait_for("return document.querySelectorAll('[data-testid=meeting-row]').length === 3 && true")
titles = s.js("return [...document.querySelectorAll('[data-testid=meeting-title]')].map(e => e.textContent)")
# A meeting without a title is named after when it was held, e.g. "Monday morning meeting" (VRT-42).
s.check(len(titles) == 3 and titles[0] == "Klantgesprek Acme" and titles[2] == "Weekly planning"
        and re.fullmatch(r"\w+ (morning|afternoon|evening|night) meeting", titles[1]), f"newest first: {titles}")
row_text = s.js("return [...document.querySelectorAll('[data-testid=meeting-row]')].map(e => e.innerText.replace(/\\s+/g, ' '))")
print("     rows:", row_text)
s.check("1:02:05" in row_text[2] and "30:30" in row_text[0], "durations formatted (1:02:05, 30:30)")
s.check("Failed" in row_text[1], "status shown when not done")
avatars = s.js("return [...document.querySelectorAll('[data-testid=meeting-row]')[0].querySelectorAll('[aria-label=Speakers] span')].map(e => e.title + '=' + e.textContent)")
s.check(avatars == ["Anna de Vries=AD", "Bram Jansen=BJ"], f"speaker avatars: {avatars}")

for theme in ("light", "dark"):
    s.js("document.documentElement.classList.toggle('dark', arguments[0] === 'dark')", theme)
    s.js("document.querySelector('[data-testid=meeting-row]').dispatchEvent(new MouseEvent('mouseover', {bubbles: true}))")
    time.sleep(0.6)  # let colour transitions finish
    s.screenshot(OUT / f"library-list-{theme}.png")
s.js("document.documentElement.classList.toggle('dark', matchMedia('(prefers-color-scheme: dark)').matches)")

# Search filters by title (React needs the native setter + input event).
set_query = ("const i = document.querySelector('[data-testid=meeting-search]');"
             "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
             "i.dispatchEvent(new Event('input', {bubbles: true}));")
s.js(set_query, "weekly")
s.wait_for("return document.querySelectorAll('[data-testid=meeting-row]').length === 1 && true", 3)
titles = s.js("return [...document.querySelectorAll('[data-testid=meeting-title]')].map(e => e.textContent)")
s.check(titles == ["Weekly planning"], f"search 'weekly' -> {titles}")
s.js(set_query, "acme zzz")
s.check(bool(s.wait_for("return document.querySelector('[data-testid=no-match]')", 3)), "no-match message")
s.screenshot(OUT / "library-nomatch.png")
s.js(set_query, "")
s.wait_for("return document.querySelectorAll('[data-testid=meeting-row]').length === 3 && true", 3)

# Delete 'Weekly planning': confirm step, then row + folder + DB rows gone.
s.js("[...document.querySelectorAll('[data-testid=meeting-row]')].find(r => r.innerText.includes('Weekly planning')).querySelector('[data-testid=delete-meeting]').click()")
s.check(bool(s.wait_for("return document.querySelector('[data-testid=confirm-delete]')", 3)), "delete asks for confirmation")
s.screenshot(OUT / "library-confirm.png")
s.js("document.querySelector('[data-testid=confirm-delete]').click()")
gone = s.wait_for("return document.querySelectorAll('[data-testid=meeting-row]').length === 2 && true", 5)
s.check(bool(gone), "row removed after delete")
s.check(not weekly_dir.exists(), f"audio folder removed ({weekly_dir.name})")
con = sqlite3.connect(DB)
left = con.execute("SELECT count(*) FROM meetings WHERE id = ?", (weekly,)).fetchone()[0]
segs = con.execute("SELECT count(*) FROM segments WHERE meeting_id = ?", (weekly,)).fetchone()[0]
others = con.execute("SELECT count(*) FROM meetings").fetchone()[0]
con.close()
s.check(left == 0 and segs == 0 and others == 2, f"DB rows removed (meeting {left}, segments {segs}, remaining meetings {others})")
s.check(all(d.exists() for t, (_, d) in ids.items() if t != "Weekly planning"), "other meeting folders untouched")

# A meeting whose folder lies outside the meetings root keeps that folder.
con = sqlite3.connect(DB)
outside = DATA / "not-a-meeting"
outside.mkdir(exist_ok=True)
(outside / "keep.txt").write_text("keep")
oid = con.execute("INSERT INTO meetings (title, started_at, dir) VALUES ('Outside', 1, ?)", (str(outside),)).lastrowid
con.commit()
con.close()
res = s.invoke("delete_meeting", {"id": oid})
s.check(res is True and (outside / "keep.txt").exists(), f"folder outside meetings/ is never deleted (result {res})")

s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
