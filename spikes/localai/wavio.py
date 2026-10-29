"""Minimal WAV helpers shared by the spike scripts."""
import struct

import numpy as np


def read_wav(data):
    """Return (rate, mono float32 samples in [-1, 1]) for PCM16 or float32 WAV bytes."""
    i, fmt, ch, sr, bits, pcm = 12, None, 1, 16000, 16, b""
    while i + 8 <= len(data):
        cid, size = data[i:i + 4], struct.unpack("<I", data[i + 4:i + 8])[0]
        body = data[i + 8:i + 8 + size]
        if cid == b"fmt ":
            fmt, ch, sr = struct.unpack("<HHI", body[:8])
            bits = struct.unpack("<H", body[14:16])[0]
        elif cid == b"data":
            pcm = body
        i += 8 + size + (size & 1)
    if fmt == 3 and bits == 32:
        x = np.frombuffer(pcm, dtype="<f4")
    elif fmt in (1, 0xFFFE) and bits == 16:
        x = np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768
    else:
        raise ValueError(f"unsupported wav format {fmt}/{bits}")
    return sr, x.reshape(-1, ch).mean(axis=1).astype(np.float32)
