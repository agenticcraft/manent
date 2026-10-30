"""VRT-29: score the libparakeet prototype on an AMI meeting the same way as
spikes/localai/eval_ami.py (WER with fillers removed; single-speaker frames attributed to
the right person), and compare its speaker embeddings with LocalAI's for the same meeting.

Usage: python3 score_ami.py <results prefix> <meeting.reference.json> <localai profiles.json>
"""
import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "localai"))
from eval_ami import frames, norm, FRAME  # noqa: E402
from spikelib import cos_dist, wer  # noqa: E402

prefix, ref = sys.argv[1], json.load(open(sys.argv[2]))
localai = json.load(open(sys.argv[3]))
asr, prof = json.load(open(prefix + ".asr.json")), json.load(open(prefix + ".profiles.json"))

ref_words = norm(" ".join(w["word"] for w in ref["words"]))
hyp_words = norm(asr["text"])
print(f"WER {wer(ref_words, hyp_words):.1%}  (ref {len(ref_words)}, hyp {len(hyp_words)} words)")

duration = max(w["end"] for w in ref["words"]) + 1
n = int(duration / FRAME) + 2
ref_masks = {s: frames([(w["start"], w["end"]) for w in ref["words"] if w["speaker"] == s], n)
             for s in sorted(set(ref["speakers"].values()))}
single = np.stack(list(ref_masks.values())).sum(axis=0) == 1
hyp_masks = defaultdict(lambda: np.zeros(n, dtype=bool))
for s in prof["segments"]:
    hyp_masks[str(s["speaker"])] |= frames([(s["start"], s["end"])], n)
mapping = {lab: max(ref_masks, key=lambda p: int((hm & ref_masks[p] & single).sum())) for lab, hm in hyp_masks.items()}
any_hyp = np.any(np.stack(list(hyp_masks.values())), axis=0)
correct = missed = total = 0
for spk, rm in ref_masks.items():
    f = rm & single
    got = np.any(np.stack([hm for lab, hm in hyp_masks.items() if mapping[lab] == spk] or [np.zeros(n, bool)]), axis=0)
    correct += int((f & got).sum())
    missed += int((f & ~any_hyp).sum())
    total += int(f.sum())
print(f"diarization: {len(hyp_masks)} speakers (reference {len(ref_masks)}), right person {correct / total:.1%}, "
      f"wrong {(total - correct - missed) / total:.1%}, missed {missed / total:.1%}")

ours = {mapping.get(str(sp["speaker"])): sp.get("embedding") for sp in prof["speaker_profiles"]["speakers"]}
for p in localai["profiles"]:
    e = ours.get(p["person"])
    if e and p.get("embedding"):
        print(f"  {p['person']}: cosine distance to LocalAI's embedding {cos_dist(np.array(e), np.array(p['embedding'])):.3f}")
