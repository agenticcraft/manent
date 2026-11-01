"""Tiny WebDriver client for tauri-driver (no extra dependencies)."""
import base64
import json
import time
import urllib.error
import urllib.request
from pathlib import Path


class Session:
    def __init__(self, app, driver="http://127.0.0.1:4444"):
        self.driver = driver
        caps = {"capabilities": {"alwaysMatch": {"tauri:options": {"application": app}}}}
        self.base = f"/session/{self._wd('POST', '/session', caps)['sessionId']}"
        self.failures = []

    def _wd(self, method, path, body=None):
        req = urllib.request.Request(
            self.driver + path,
            data=json.dumps(body).encode() if body is not None else None,
            method=method,
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}").get("value")
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"{method} {path} -> {e.code}: {e.read().decode()[:300]}") from None

    def js(self, script, *args):
        return self._wd("POST", f"{self.base}/execute/sync", {"script": script, "args": list(args)})

    def js_async(self, script, *args):
        return self._wd("POST", f"{self.base}/execute/async", {"script": script, "args": list(args)})

    def invoke(self, command, args=None):
        """Call a Tauri command from inside the app; errors come back as {'error': ...}."""
        return self.js_async(
            "const done = arguments[arguments.length - 1];"
            "window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1] || {})"
            ".then(done, e => done({error: String(e)}));",
            command,
            args or {},
        )

    def wait_for(self, script, timeout=15.0, *args):
        """Poll a JS expression (with optional arguments) until it returns something truthy."""
        end = time.time() + timeout
        while time.time() < end:
            try:
                v = self.js(script, *args)
                if v:
                    return v
            except RuntimeError:
                pass
            time.sleep(0.3)
        return None

    def screenshot(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(self._wd("GET", f"{self.base}/screenshot")))

    def check(self, cond, msg):
        print(("ok   " if cond else "FAIL ") + msg)
        if not cond:
            self.failures.append(msg)

    def close(self):
        try:
            self._wd("DELETE", self.base)
        except RuntimeError:
            pass
