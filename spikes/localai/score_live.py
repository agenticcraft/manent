"""VRT-32: score a live run (results/*-live-events.json) against a truth file with per-turn text.

Usage: score_live.py <live-events.json> <truth.json>

Prints WER of the completed transcripts overall and per language (turn text matched to
completed transcripts by time), final-transcript lag after each reference turn and how
many caption deltas arrived.
"""
import json
import sys

from eval_ami import norm
from spikelib import wer

events = json.load(open(sys.argv[1]))
truth = json.load(open(sys.argv[2]))["segments"]
done = [e for e in events if e.get("type", "").endswith("transcription.completed")]
deltas = [e for e in events if e.get("type", "").endswith("transcription.delta") and e.get("delta", "").strip()]
errors = [e for e in events if e.get("type") == "error"]
print(f"completed: {len(done)}, caption deltas: {len(deltas)}, errors: {len(errors)}")
for e in errors[:3]:
    print("  error:", json.dumps(e)[:200])

ref_all = norm(" ".join(t["text"] for t in truth))
hyp_all = norm(" ".join(e.get("transcript", "") for e in done))
print(f"WER all: {wer(ref_all, hyp_all):.1%} (ref {len(ref_all)}, hyp {len(hyp_all)} words)")

# assign each completed transcript to the reference turn that ended most recently before it
by_lang = {}
lags = []
for t in truth:
    nxt = [e for e in done if e["t"] >= t["end"] - 0.5]
    if nxt:
        lags.append(round(nxt[0]["t"] - t["end"], 2))
for e in done:
    prior = [t for t in truth if t["end"] - 0.5 <= e["t"]]
    if prior:
        lang = prior[-1].get("lang", "?")
        by_lang.setdefault(lang, [[], []])[1] += norm(e.get("transcript", ""))
for t in truth:
    by_lang.setdefault(t.get("lang", "?"), [[], []])[0] += norm(t["text"])
for lang, (r, h) in by_lang.items():
    print(f"  {lang}: WER {wer(r, h):.1%} (ref {len(r)} words)")
print("final lag after each turn end (s):", lags)
for e in done:
    print(f"  t={e['t']:6.2f}  {e.get('transcript', '')[:120]}")
