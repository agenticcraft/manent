"""Playback smoke check: record a few seconds, stop, check mix.opus, play it in the app.

Usage: python3 scripts/smoke-playback.py <app binary> <data dir> <screenshot dir> [<tone.wav>]

tauri-driver must run with MANENT_DATA_DIR=<data dir> (an empty temp folder). Needs audio
hardware; the optional WAV is played (quietly) so the mix contains a known tone.
"""
import math
import sqlite3
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
TONE = sys.argv[4] if len(sys.argv) > 4 else None

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
player = subprocess.Popen(["paplay", "--volume=20000", TONE]) if TONE else None
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
time.sleep(5)
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
if player:
    player.terminate()

con = sqlite3.connect(DATA / "manent.db")
meeting_id, folder, duration_ms = con.execute("SELECT id, dir, duration_ms FROM meetings").fetchone()
con.close()
mix = Path(folder) / "mix.opus"
s.check(mix.is_file(), f"mix.opus created on Stop ({mix.stat().st_size if mix.is_file() else 0} bytes)")
probe = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_name,channels,sample_rate",
                        "-of", "default=nw=1", str(mix)], capture_output=True, text=True).stdout
print("     ffprobe:", probe.replace("\n", " "))
mix_seconds = float(next(l.split("=")[1] for l in probe.splitlines() if l.startswith("duration=")))
s.check("codec_name=opus" in probe and "channels=1" in probe, "mono Opus stream")
s.check(abs(mix_seconds - duration_ms / 1000) < 0.1, f"mix length {mix_seconds:.2f}s matches the meeting ({duration_ms / 1000:.2f}s)")
if TONE:
    def tone_strength(path):
        """440 Hz strength in seconds 1-2 of a file (mono, 48 kHz)."""
        pcm = subprocess.run(["ffmpeg", "-loglevel", "error", "-i", str(path), "-ac", "1", "-ar", "48000", "-f", "s16le", "-"],
                             capture_output=True, check=True).stdout
        x = [v / 32768 for v in struct.unpack(f"<{len(pcm) // 2}h", pcm)][48000:96000]
        k = 2 * math.cos(2 * math.pi * 440 / 48000)
        s1 = s2 = 0.0
        for v in x:
            s1, s2 = v + k * s1 - s2, s1
        return math.sqrt(max(s1 * s1 + s2 * s2 - k * s1 * s2, 0)) / len(x)
    # The mix also carries the room (mic), so compare with the system track itself.
    in_system, in_mix = tone_strength(Path(folder) / "system.flac"), tone_strength(mix)
    s.check(in_system > 0 and in_mix > 0.5 * in_system,
            f"system audio reaches the mix (440 Hz: system track {in_system:.4f}, mix {in_mix:.4f})")

# Open the meeting and play it.
s.js("document.querySelector('[data-testid=open-meeting]').click()")
ready = s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 10)
s.check(bool(ready) and abs(ready - mix_seconds) < 0.2, f"player loaded the mix (duration {ready})")
s.js("document.querySelector('[data-testid=play-toggle]').click()")
time.sleep(1.5)
t_play = s.js("return document.querySelector('[data-testid=audio]').currentTime")
playing = s.js("return !document.querySelector('[data-testid=audio]').paused")
s.check(playing and t_play > 0.8, f"play advances ({t_play:.2f}s after 1.5 s)")
s.screenshot(OUT / "meeting-playing.png")
s.js("document.querySelector('[data-testid=play-toggle]').click()")
time.sleep(0.3)
p1 = s.js("return document.querySelector('[data-testid=audio]').currentTime")
time.sleep(1.0)
p2 = s.js("return document.querySelector('[data-testid=audio]').currentTime")
s.check(abs(p2 - p1) < 0.05, f"pause holds the position ({p1:.2f} -> {p2:.2f})")
s.js("const r = document.querySelector('[data-testid=seek]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(r, '3.5');"
     "r.dispatchEvent(new Event('input', {bubbles: true}));")
time.sleep(0.5)
pos = s.js("return document.querySelector('[data-testid=audio]').currentTime")
shown = s.js("return document.querySelector('[data-testid=player-time]').textContent")
s.check(abs(pos - 3.5) < 0.1 and shown == "0:03", f"seek jumps to 3.5 s (audio {pos:.2f}s, shown {shown})")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "meeting-light.png")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
