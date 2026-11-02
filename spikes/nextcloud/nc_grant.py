"""Test helper: approve a Nextcloud Login Flow v2 as a user would in the browser (VRT-38).

Usage: python3 -I nc_grant.py <login url> <user> <password>

Opens the flow page, logs in with the login form (CSRF token and Origin, as a browser sends
them) and presses "Grant access". Only for a local test instance: real users do this in their
own browser, which Veritas opens for them.
"""
import base64
import json
import re
import sys
from urllib.parse import urlsplit

import requests

login_url, user, password = sys.argv[1:4]
base = "{0.scheme}://{0.netloc}".format(urlsplit(login_url))
s = requests.Session()
s.headers["Origin"] = base


def state(html, key):
    m = re.search(rf'id="initial-state-{key}" value="([^"]*)"', html)
    return json.loads(base64.b64decode(m.group(1))) if m else None


def token(html):
    return re.search(r'data-requesttoken="([^"]*)"', html).group(1)


flow = s.get(login_url)
auth = state(flow.text, "core-loginFlowAuth")
grant_path = auth["loginRedirectUrl"][len(base):]
login_page = s.get(f"{base}/login", params={"redirect_url": grant_path})
page = s.post(f"{base}/login", data={
    "user": user, "password": password, "requesttoken": token(login_page.text),
    "timezone": "Europe/Amsterdam", "timezone_offset": "1", "redirect_url": grant_path,
})
errors = state(page.text, "core-loginErrors")
if errors:
    sys.exit(f"login failed: {errors}")
grant = state(page.text, "core-loginFlowGrant") or {}
r = s.post(f"{base}/login/v2/grant", data={
    "stateToken": grant.get("stateToken", auth["stateToken"]), "requesttoken": token(page.text),
}, allow_redirects=False)
print("granted" if r.status_code in (200, 303) else f"grant failed: HTTP {r.status_code}")
