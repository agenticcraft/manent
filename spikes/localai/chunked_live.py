"""VRT-32: simulate app-side VAD chunking for live captions.

Usage: chunked_live.py <wav> <reference.json> [--min 4] [--max 8] [--language nl] [--model parakeet-cpp-bundle-standard]

Cuts the audio at the quietest 20 ms frame between --min and --max seconds after the
previous cut (so a chunk never waits for a pause), sends each chunk to
/v1/audio/transcriptions as the app would, and reports:
  - WER of the joined chunk transcripts vs the reference text
  - latency: a word spoken at time t appears at (chunk end + processing time); reported as
    the wait after each reference segment's end, assuming chunks are sent as soon as they close
"""
import io
import json
import sys
import tempfile
import time
import wave
from pathlib import Path

import numpy as np

from eval_ami import norm
from spikelib import post_file, wer
from wavio import read_wav

args = sys.argv[1:]
wav, ref_path = args[0], args[1]
MIN = float(args[args.index("--min") + 1]) if "--min" in args else 4.0
MAX = float(args[args.index("--max") + 1]) if "--max" in args else 8.0
LANG = args[args.index("--language") + 1] if "--language" in args else None
MODEL = args[args.index("--model") + 1] if "--model" in args else "parakeet-cpp-bundle-standard"

sr, x = read_wav(open(wav, "rb").read())
frame = int(0.02 * sr)
rms = np.sqrt(np.convolve(x ** 2, np.ones(frame) / frame, mode="same"))
cuts, pos = [0], 0
while pos + int(MAX * sr) < len(x):
    lo, hi = pos + int(MIN * sr), pos + int(MAX * sr)
    pos = lo + int(np.argmin(rms[lo:hi]))
    cuts.append(pos)
cuts.append(len(x))

texts, ready = [], []
clock = 0.0  # simulated wall clock: a chunk can start when it has been recorded and the previous one is done
with tempfile.TemporaryDirectory() as tmp:
    for i, (a, b) in enumerate(zip(cuts, cuts[1:])):
        path = Path(tmp) / f"chunk{i}.wav"
        with wave.open(str(path), "wb") as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(sr)
            w.writeframes((np.clip(x[a:b], -1, 1) * 32767).astype("<i2").tobytes())
        fields = {"model": MODEL}
        if LANG:
            fields["language"] = LANG
        t0 = time.time()
        res = post_file("/v1/audio/transcriptions", path, **fields)
        took = time.time() - t0
        clock = max(clock, b / sr) + took
        texts.append(res.get("text", ""))
        ready.append((b / sr, clock))

ref = json.load(open(ref_path))["segments"]
hyp = norm(" ".join(texts))
print(f"{len(cuts) - 1} chunks ({MIN}-{MAX} s), WER vs reference {wer(norm(' '.join(s['text'] for s in ref)), hyp):.1%}")
waits = []
for s in ref:
    done = next((r for end, r in ready if end >= s["end"] - 0.05), None)
    if done is not None:
        waits.append(done - s["end"])
print(f"text appears after speech: median {np.median(waits):.1f}s, 90th pct {np.percentile(waits, 90):.1f}s, max {max(waits):.1f}s")
proc = [r - e for e, r in ready]
print(f"processing per chunk: median {np.median(proc):.2f}s, max {max(proc):.2f}s")
print("sample:", " | ".join(texts[:4])[:300])
