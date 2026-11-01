"""Meeting transcript smoke check on the real app (VRT-20): search, click-to-seek, highlight.

Usage: python3 scripts/smoke-transcript.py <app binary> <data dir> <screenshot dir> <query>=<expected word> ...

tauri-driver must run with VERITAS_DATA_DIR=<data dir>, a folder with transcribed meetings
(e.g. after smoke-import.py). For each query (typed in the Library search, e.g.
"technologieen=technologieën" to check that diacritics are ignored) a transcript match must
appear, highlighting the expected word; the first match opens its meeting at that moment.
There a word is clicked: playback jumps there, and the word being played is highlighted.
"""
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
QUERIES = [q.split("=", 1) for q in sys.argv[4:]]

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-search]')", 15)
first_hit = None
for query, expected in QUERIES:
    s.js("const i = document.querySelector('[data-testid=meeting-search]');"
         "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
         "i.dispatchEvent(new Event('input', {bubbles: true}));", query)
    # Results follow typing after a short pause: wait for this query's matches.
    marks = s.wait_for("const m = [...document.querySelectorAll('[data-testid=transcript-hit] mark')].map(x => x.textContent);"
                       "return m.some(x => x.toLowerCase().includes(arguments[0].toLowerCase())) ? m : null", 5, expected)
    s.check(bool(marks) and any(expected.lower() in m.lower() for m in marks),
            f"search '{query}' finds '{expected}' (highlighted: {marks[:4] if marks else marks})")
    if first_hit is None and marks:
        first_hit = s.js("const h = document.querySelector('[data-testid=transcript-hit]'); return [Number(h.dataset.meetingId), Number(h.dataset.startMs)]")
        s.screenshot(OUT / "search-dark.png")
        s.js("document.documentElement.classList.remove('dark')")
        time.sleep(0.5)
        s.screenshot(OUT / "search-light.png")
        s.js("document.documentElement.classList.add('dark')")

# Open the first match shown (of the last query): the meeting opens at that moment.
meeting_id, at_ms = s.js("const h = document.querySelector('[data-testid=transcript-hit]'); return [Number(h.dataset.meetingId), Number(h.dataset.startMs)]")
s.js("document.querySelector('[data-testid=transcript-hit]').click()")
s.wait_for("return document.querySelectorAll('[data-testid=word]').length", 10)
ready = s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 10)
cued = s.wait_for(f"const a = document.querySelector('[data-testid=audio]'); return Math.abs(a.currentTime * 1000 - {at_ms}) < 300 && a.currentTime", 5)
s.check(bool(cued), f"the match opens with the player at {at_ms / 1000:.1f} s ({cued})")
in_view = s.js(f"const b = [...document.querySelectorAll('[data-block-start]')].filter(b => Number(b.dataset.blockStart) <= {at_ms}).pop();"
               "const r = b.getBoundingClientRect(); return r.top >= 0 && r.top < window.innerHeight")
s.check(in_view, "the matching block is scrolled into view")
# The waveform is computed on first open (a few seconds), then cached.
bars = s.wait_for("const w = document.querySelector('[data-testid=waveform]'); return w ? Number(w.dataset.bars) : 0", 20)
overflow = s.js("const m = document.querySelector('main'); return m.scrollWidth - m.clientWidth")
s.check(overflow <= 0, f"no horizontal overflow ({overflow} px)")
s.check(bars == 600, f"waveform drawn ({bars} bars)")

# Click a word some way into the transcript: playback jumps there and plays.
target = s.js("const w = [...document.querySelectorAll('[data-testid=word]')]; const x = w[Math.floor(w.length / 2)];"
              "x.scrollIntoView({block: 'center'}); return [Number(x.dataset.wordMs), x.textContent]")
time.sleep(0.3)
s.js("const w = [...document.querySelectorAll('[data-testid=word]')]; w[Math.floor(w.length / 2)].click()")
time.sleep(0.4)
t0 = s.js("return document.querySelector('[data-testid=audio]').currentTime")
s.check(abs(t0 * 1000 - target[0]) < 500, f"clicking '{target[1]}' seeks to {target[0] / 1000:.2f} s (audio {t0:.2f} s)")
time.sleep(2.0)
state = s.js("const a = document.querySelector('[data-testid=audio]'); const w = document.querySelector('[data-testid=word][data-active=true]');"
             "const b = document.querySelector('[data-testid=caption-block][data-current=true]');"
             "return [a.paused, a.currentTime, w && Number(w.dataset.wordMs), w && w.textContent, !!(b && w && b.contains(w))]")
paused, now, word_ms, word, in_block = state
s.check(not paused and now > t0 + 1.0, f"playing on ({t0:.2f} -> {now:.2f} s)")
s.check(word_ms is not None and word_ms <= now * 1000 + 300 and now * 1000 - word_ms < 3000,
        f"the word being played is highlighted ('{word}' at {word_ms and word_ms / 1000:.2f} s, audio at {now:.2f} s)")
s.check(in_block, "its block is highlighted too")
s.screenshot(OUT / "meeting-playing-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.5)
s.screenshot(OUT / "meeting-playing-light.png")
s.js("document.querySelector('[data-testid=play-toggle]').click()")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
