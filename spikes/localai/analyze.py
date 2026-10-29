"""Compare a LocalAI transcription (verbose_json) with the meeting ground truth.

Usage: analyze.py <transcription.json> <truth.json>
Prints segments, overall and per-language word error rate (WER), and speaker labels per turn.
"""
import json
import re
import sys
from collections import Counter

from spikelib import wer


def norm(text):
    return re.sub(r"[^\w\s']", " ", text.lower()).split()


def words_in(words, start, end):
    """Words whose midpoint falls inside [start, end]."""
    return [w for w in words if start <= (w["start"] + w["end"]) / 2 <= end]


hyp, truth = json.load(open(sys.argv[1])), json.load(open(sys.argv[2]))
segs = hyp.get("segments") or []
# LocalAI nests words per segment ({"start","end","text"}) and labels speakers per segment.
words = [{"word": w["text"], "start": w["start"], "end": w["end"], "speaker": s.get("speaker")}
         for s in segs for w in s.get("words") or []]
print(f"language={hyp.get('language')} duration={hyp.get('duration')} segments={len(segs)} words={len(words)}")
print("\nSegments:")
for s in segs:
    print(f"  [{s['start']:6.2f}-{s['end']:6.2f}] spk={s.get('speaker')!s:>4}  {s['text'].strip()[:100]}")

print("\nPer turn (reference vs hypothesis):")
totals = {"nl": [[], []], "en": [[], []]}
for t in truth["segments"]:
    ws = words_in(words, t["start"] - 0.3, t["end"] + 0.3) if words else []
    hyp_text = " ".join(w["word"] for w in ws)
    spk = Counter(str(w.get("speaker")) for w in ws).most_common(2)
    e = wer(norm(t["text"]), norm(hyp_text))
    totals[t["lang"]][0] += norm(t["text"])
    totals[t["lang"]][1] += norm(hyp_text)
    print(f"  {t['start']:6.2f} {t['voice']} WER={e:5.1%} speakers={spk}")
    print(f"      ref: {t['text'][:110]}")
    print(f"      hyp: {hyp_text[:110]}")

print("\nWER by language:")
for lang, (r, h) in totals.items():
    print(f"  {lang}: {wer(r, h):.1%} over {len(r)} reference words")
all_r = totals["nl"][0] + totals["en"][0]
all_h = totals["nl"][1] + totals["en"][1]
print(f"  all: {wer(all_r, all_h):.1%}")
