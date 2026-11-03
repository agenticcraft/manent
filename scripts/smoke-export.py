"""Transcript export smoke check on the real app (VRT-21).

Usage: python3 scripts/smoke-export.py <app binary> <data dir> <screenshot dir> <out dir>

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a folder with transcribed meetings
(e.g. after smoke-import.py or smoke-naming.py). Opens a meeting with named people, checks the
Export menu, and exports all four formats into <out dir> through the same command the menu
uses after the save dialog (the native dialog itself can't be driven). Checks each file:
speaker names as in the app, timestamps, SRT cue timing, JSON words.
"""
import json
import re
import sqlite3
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, DEST = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
DEST.mkdir(parents=True, exist_ok=True)

con = sqlite3.connect(DATA / "manent.db")
# A meeting whose final transcript names at least one person.
row = con.execute("""SELECT s.meeting_id, m.title FROM segments s JOIN meetings m ON m.id = s.meeting_id
                     WHERE s.pass = 'final' AND s.speaker_id IS NOT NULL
                     GROUP BY s.meeting_id ORDER BY s.meeting_id DESC LIMIT 1""").fetchone()
meeting, title = row
names = {n for (n,) in con.execute("""SELECT DISTINCT sp.name FROM segments s JOIN speakers sp ON sp.id = s.speaker_id
                                      WHERE s.meeting_id = ?""", (meeting,))}
con.close()

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
s.js("document.querySelector(`[data-testid=meeting-row][data-meeting-id='${arguments[0]}'] [data-testid=open-meeting]`).click()",
     meeting)
s.wait_for("return document.querySelector('[data-testid=export]')", 10)
s.js("document.querySelector('[data-testid=export]').click()")
items = s.wait_for("return [...document.querySelectorAll('[data-testid=export-menu] [data-format]')].map(b => b.dataset.format)", 3)
s.check(items == ["md", "txt", "srt", "json"], f"Export menu offers {items}")
s.screenshot(OUT / "export-menu-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.5)
s.screenshot(OUT / "export-menu-light.png")
date = s.js("return document.querySelector('[data-testid=meeting-heading]').nextElementSibling.textContent.split(' · ')[0]")

for fmt in ["md", "txt", "srt", "json"]:
    path = DEST / f"meeting-{meeting}.{fmt}"
    r = s.invoke("export_meeting", {"id": meeting, "format": fmt, "path": str(path), "dateLabel": date})
    s.check(r is None and path.is_file(), f"{fmt}: written ({path.stat().st_size if path.is_file() else 0} bytes)")
s.close()

md = (DEST / f"meeting-{meeting}.md").read_text()
s.check(md.startswith(f"# {title or 'Untitled meeting'}\n") and date in md, "Markdown: title and date")
s.check(all(f"**{n}**" in md for n in names), f"Markdown: named people {sorted(names)}")
txt = (DEST / f"meeting-{meeting}.txt").read_text()
lines = [line for line in txt.splitlines() if line.startswith("[")]
s.check(lines and all(re.match(r"^\[\d+:\d{2}(:\d{2})?\] [^:]+: .+", line) for line in lines), f"TXT: {len(lines)} '[time] Speaker: text' lines")
srt = (DEST / f"meeting-{meeting}.srt").read_text()
cues = re.findall(r"(\d+)\n(\d\d):(\d\d):(\d\d),(\d{3}) --> (\d\d):(\d\d):(\d\d),(\d{3})\n(.+)\n", srt)
ms = lambda h, m, sec, f: ((int(h) * 60 + int(m)) * 60 + int(sec)) * 1000 + int(f)  # noqa: E731
lengths = [ms(*c[5:9]) - ms(*c[1:5]) for c in cues]
s.check(cues and [int(c[0]) for c in cues] == list(range(1, len(cues) + 1)), f"SRT: {len(cues)} numbered cues")
s.check(all(0 < n <= 7000 for n in lengths), f"SRT: cues last at most 7 s (longest {max(lengths) if lengths else 0} ms)")
doc = json.loads((DEST / f"meeting-{meeting}.json").read_text())
words = sum(len(b["words"]) for b in doc["blocks"])
s.check(doc["blocks"] and words > 0 and names <= set(doc["speakers"]), f"JSON: {len(doc['blocks'])} blocks, {words} words, speakers {doc['speakers']}")
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; files in {DEST}, screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
