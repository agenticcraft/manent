"""VRT-38 end-to-end check of the spike against a local Nextcloud (never a real account).

Usage: python3 -I e2e.py <nc-spike binary> <server> <user> <password> <work dir> <speech.wav>

Start the server first, e.g.:
  docker run -d --name veritas-nc -p 127.0.0.1:8471:80 -e SQLITE_DATABASE=nextcloud \\
    -e NEXTCLOUD_ADMIN_USER=admin -e NEXTCLOUD_ADMIN_PASSWORD=<pw> \\
    -e NEXTCLOUD_TRUSTED_DOMAINS="127.0.0.1 localhost" nextcloud:apache
  docker exec -u www-data -e OC_PASS=<password> veritas-nc php occ user:add --password-from-env <user>

Steps: Login Flow v2 (approved by nc_grant.py, as the user would in the browser), app password
in the keychain only; a 1-hour Opus recording uploaded in 5 MB chunks, interrupted after one
chunk and resumed; the server copy is identical; a download interrupted after 3 MB resumes;
ETag watching sees the desktop's transcript.md appear and the recording move from the inbox\nto its meeting folder; revoking the app
password locks the client out and empties the keychain.
"""
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

BIN, SERVER, USER, PASSWORD, WORK, SPEECH = sys.argv[1:7]
WORK = Path(WORK)
WORK.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, NC_SPIKE_STATE=str(WORK / "nc-spike.json"))
HERE = Path(__file__).parent
failures = []


def check(cond, msg):
    print(f"{'ok  ' if cond else 'FAIL'} {msg}", flush=True)
    if not cond:
        failures.append(msg)


def nc(*args, ok=True):
    r = subprocess.run([BIN, *args], capture_output=True, text=True, env=ENV)
    if ok and r.returncode != 0:
        raise SystemExit(f"nc-spike {' '.join(args)} failed: {r.stderr.strip()}")
    return r


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


# A 1-hour recording as the phone would send it (Opus, 32 kbit/s mono).
rec = WORK / "phone-recording.opus"
if not rec.exists():
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-stream_loop", "-1", "-i", SPEECH, "-t", "3600",
                    "-ac", "1", "-c:a", "libopus", "-b:a", "32k", str(rec)], check=True)
print(f"     recording: {rec.stat().st_size / 1e6:.1f} MB")

# 1. Login Flow v2.
login = subprocess.Popen([BIN, "login", SERVER, "--no-browser"], stdout=subprocess.PIPE, text=True, env=ENV)
url = login.stdout.readline().split(" ", 1)[1].strip()
grant = subprocess.run([sys.executable, "-I", str(HERE / "nc_grant.py"), url, USER, PASSWORD], capture_output=True, text=True)
out = login.communicate(timeout=30)[0]
check(login.returncode == 0 and "logged in" in out, f"Login Flow v2: {grant.stdout.strip()}; {out.strip()}")
state = json.loads((WORK / "nc-spike.json").read_text())
check(set(state) == {"server", "login"}, f"local state has no password: {state}")
stored = subprocess.run(["secret-tool", "search", "--all", "--unlock", "service", "veritas-spike-nextcloud"],
                        capture_output=True, text=True)
secret = next((line.split("= ", 1)[1] for line in stored.stdout.splitlines() if line.startswith("secret = ")), "")
check(len(secret) > 20, "app password is in the keychain (Secret Service)")

# 2. Folder layout (from scratch: earlier runs leave files behind).
nc("rm", "Veritas")
for d in ["Veritas", "Veritas/Inbox", "Veritas/2026-11-02 Planning"]:
    nc("mkdir", d)
nc("mkdir", "Veritas")  # again: already there is fine

# 3. Chunked upload, interrupted after one chunk, then resumed.
remote = "Veritas/Inbox/phone-recording.opus"
r1 = nc("upload", str(rec), remote, "--chunk-mb", "5", "--stop-after", "1").stdout
check("sent=1" in r1 and "finished=false" in r1, f"interrupted upload: {r1.strip()}")
r2 = nc("upload", str(rec), remote, "--chunk-mb", "5").stdout
check("skipped=1" in r2 and "finished=true" in r2, f"resumed upload skips the chunk already there: {r2.strip()}")
full = WORK / "check-full.opus"
full.unlink(missing_ok=True)
nc("download", remote, str(full))
check(sha(full) == sha(rec), "the file on the server is identical (sha256)")

# 4. Interrupted download resumes with Range (If-Match on the ETag).
part, dest = WORK / "resume.opus.part", WORK / "resume.opus"
dest.unlink(missing_ok=True)
part.write_bytes(rec.read_bytes()[: 3 * 1024 * 1024])
etag = nc("etag", remote).stdout.strip()
(WORK / "resume.opus.etag").write_text(f'"{etag}"')
d = nc("download", remote, str(dest)).stdout
check(sha(dest) == sha(rec) and f"resumed at {3 * 1024 * 1024}" in d, f"download resumed after 3 MB: {d.strip()}")

# 5. Watching by ETag: the desktop adds a transcript, the inbox recording is moved away.
before = nc("etag", "Veritas").stdout.strip()
watch = subprocess.Popen([BIN, "watch", "Veritas", "--interval", "2", "--rounds", "4"],
                         stdout=subprocess.PIPE, text=True, env=ENV)
time.sleep(3)
transcript = "---\nveritas: 1\nid: 7\ntitle: \"Planning\"\n---\n\n# Planning\n\n**Anna** · 0:01\n\nGoedemorgen.\n"
nc("put", "Veritas/2026-11-02 Planning/transcript.md", transcript)
moved = "Veritas/2026-11-02 Planning/phone-recording.opus"
nc("mv", remote, moved)
lines = watch.communicate(timeout=60)[0].splitlines()
print("     watch:", lines)
after = nc("etag", "Veritas").stdout.strip()
check(before != after, "the Veritas folder's ETag changed (propagated from below)")
check("ADDED Veritas/2026-11-02 Planning/transcript.md" in lines, "watch sees the new transcript.md")
check(not any(line.startswith("CHANGED Veritas/Inbox") and line.endswith("Inbox") for line in lines), "folders are not reported as changed files")
check(f"REMOVED {remote}" in lines and f"ADDED {moved}" in lines,
      "watch sees the recording move from the inbox to its meeting folder (server-side MOVE)")
check(any(line == "UNCHANGED" for line in lines), "a quiet round costs one Depth-0 PROPFIND")
got = WORK / "transcript.md"
got.unlink(missing_ok=True)
nc("download", "Veritas/2026-11-02 Planning/transcript.md", str(got))
check(got.read_text() == transcript, "transcript.md downloaded as written")

# 6. Revoke.
rv = nc("revoke").stdout
check("REVOKED" in rv, rv.strip())
old = subprocess.run(["curl", "-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "PROPFIND", "-u", f"{USER}:{secret}",
                      f"{SERVER}/remote.php/dav/files/{USER}/Veritas"], capture_output=True, text=True).stdout
check(old == "401", f"the server refuses the revoked app password (HTTP {old})")
gone = subprocess.run(["secret-tool", "search", "--all", "service", "veritas-spike-nextcloud"], capture_output=True, text=True)
check("veritas-spike-nextcloud" not in gone.stdout, "keychain entry removed")

print(f"\n{'PASS' if not failures else 'FAILED: ' + '; '.join(failures)}")
sys.exit(1 if failures else 0)
