"""VRT-31: cross-session naming with in-app matching (doc-2 §1.4).

Usage: naming_cross.py <enroll-profiles.json> <test-profiles.json> [<test-profiles.json> ...]

Enrolls every slot of the first meeting (person known from eval_ami.py's mapping) and
names the slots of the other meetings by the nearest enrolled sample:
  distance <= AUTO -> auto-name, <= SUGGEST -> suggest, else unnamed.
A slot is 'correct' when its predicted person equals the person it maps to; for
meetings with other people every slot should end up unnamed.
"""
import json
import sys

from spikelib import cos_dist

AUTO, SUGGEST, MIN_CLEAN = 0.45, 0.60, 5.0

enroll = json.load(open(sys.argv[1]))
samples = [(p["person"], p["embedding"]) for p in enroll["profiles"]
           if p["embedding"] and p["clean_duration"] >= MIN_CLEAN]
print(f"enrolled from {enroll['meeting']}: {[s[0] for s in samples]}")

for path in sys.argv[2:]:
    test = json.load(open(path))
    print(f"\n{test['meeting']}:")
    for p in test["profiles"]:
        if not p["embedding"]:
            print(f"  slot {p['slot']}: no embedding")
            continue
        dists = sorted((cos_dist(p["embedding"], emb), person) for person, emb in samples)
        d, nearest = dists[0]
        band = "auto" if d <= AUTO else "suggest" if d <= SUGGEST else "unnamed"
        predicted = nearest if band != "unnamed" else None
        known = p["person"] in {s[0] for s in samples}
        if known:
            verdict = "ok" if predicted == p["person"] and band == "auto" else \
                      "ok (needs confirm)" if predicted == p["person"] else \
                      "missed" if predicted is None else "WRONG"
        else:
            verdict = "ok (stranger unnamed)" if predicted is None else "FALSE NAME"
        others = ", ".join(f"{q} {x:.3f}" for x, q in dists[1:3])
        print(f"  slot {p['slot']} truth={p['person']:7} clean={p['clean_duration']:5.1f}s  nearest={nearest} {d:.3f} "
              f"-> {band:7} {verdict:22} (next: {others})")
