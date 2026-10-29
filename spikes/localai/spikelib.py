"""Helpers shared by the LocalAI spike scripts."""
import json
import urllib.request
import uuid

import numpy as np

URL = "http://127.0.0.1:8181"


def wer(ref, hyp):
    """Word error rate via Levenshtein distance over words."""
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / max(len(ref), 1)


def post_json(path, body):
    req = urllib.request.Request(URL + path, json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as r:
        return json.loads(r.read() or b"{}")


def post_file(path, wav, **fields):
    """multipart/form-data upload without external dependencies."""
    boundary = uuid.uuid4().hex
    body = b""
    for k, v in fields.items():
        for item in v if isinstance(v, list) else [v]:
            body += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{item}\r\n".encode()
    body += (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{wav.name}\"\r\n"
             f"Content-Type: audio/wav\r\n\r\n").encode() + wav.read_bytes() + f"\r\n--{boundary}--\r\n".encode()
    req = urllib.request.Request(URL + path, body, {"Content-Type": f"multipart/form-data; boundary={boundary}"})
    with urllib.request.urlopen(req, timeout=600) as r:
        return json.loads(r.read())


def cos_dist(a, b):
    a, b = np.asarray(a), np.asarray(b)
    return float(1 - a @ b / (np.linalg.norm(a) * np.linalg.norm(b)))
