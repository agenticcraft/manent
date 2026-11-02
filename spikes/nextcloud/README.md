# Nextcloud sync spike (VRT-38)

A small WebDAV client for Nextcloud over ureq, and an end-to-end check against a local server. The findings and the decision are in decision-7.

- `src/dav.rs`: PROPFIND with ETags, GET with Range and If-Match, PUT, MKCOL, MOVE, DELETE, and chunked upload v2 with resume.
- `src/main.rs`: the `nc-spike` CLI, covering:
  - Login Flow v2, with the app password in the OS keychain;
  - upload and download;
  - watch, which reports what changed below a folder by ETag;
  - revoke.
- `nc_grant.py`: approves a login flow over HTTP, as the user would in the browser. Use it only against a local test server.
- `e2e.py`: the whole scenario. Its top comment shows how to start the container and create the test user.

```
cargo build
python3 -I e2e.py target/debug/nc-spike http://127.0.0.1:8471 vtest <password> <work dir> <speech.wav>
```

Never point this at a real account: the test empties the `Veritas` folder of the user it logs in as.
