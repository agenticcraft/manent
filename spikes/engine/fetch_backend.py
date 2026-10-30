"""VRT-29: download a LocalAI parakeet-cpp backend image from quay.io without docker and
extract its lib/ folder (libparakeet + runtime libs). Every blob is checked against its
sha256 digest.

Usage: python3 -I fetch_backend.py <tag> <os> <arch> <out dir>
       e.g. latest-cpu-parakeet-cpp linux amd64 lib-linux-amd64
"""
import hashlib
import io
import json
import sys
import tarfile
import urllib.request
from pathlib import Path

REPO = "go-skynet/local-ai-backends"
REGISTRY = "https://quay.io/v2"
ACCEPT = ", ".join([
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
    "application/vnd.docker.distribution.manifest.v2+json",
])


def get(path, accept=ACCEPT):
    req = urllib.request.Request(f"{REGISTRY}/{REPO}/{path}", headers={"Accept": accept, "User-Agent": "veritas-spike"})
    with urllib.request.urlopen(req, timeout=300) as r:
        return r.read()


def main(tag, os_name, arch, out):
    out = Path(out)
    index = json.loads(get(f"manifests/{tag}"))
    manifest = index
    if "manifests" in index:
        match = [m for m in index["manifests"] if m.get("platform", {}).get("os") == os_name and m["platform"].get("architecture") == arch]
        if not match:
            raise SystemExit(f"no {os_name}/{arch} in {tag}: {[m.get('platform') for m in index['manifests']]}")
        manifest = json.loads(get(f"manifests/{match[0]['digest']}"))
    total = 0
    out.mkdir(parents=True, exist_ok=True)
    for layer in manifest["layers"]:
        digest = layer["digest"]
        blob = get(f"blobs/{digest}", accept="*/*")
        actual = "sha256:" + hashlib.sha256(blob).hexdigest()
        if actual != digest:
            raise SystemExit(f"digest mismatch for {digest}: {actual}")
        total += len(blob)
        print(f"layer {digest[:19]} {len(blob) / 1048576:.1f} MB ok")
        with tarfile.open(fileobj=io.BytesIO(blob)) as tar:
            for m in tar.getmembers():
                name = m.name.lstrip("./")
                if (name.startswith("lib/") or name in ("run.sh", "metadata.json")) and (m.isfile() or m.issym()):
                    if ".." in Path(name).parts:
                        continue
                    tar.extract(m, out, filter="data")
    print(f"downloaded {total / 1048576:.1f} MB compressed")
    for f in sorted(out.rglob("*")):
        if f.is_file():
            print(f"  {f.relative_to(out)}  {f.stat().st_size / 1048576:.2f} MB")


if __name__ == "__main__":
    main(*sys.argv[1:5])
