---
id: decision-7
title: >-
  Nextcloud sync: raw WebDAV over ureq, ETag polling, keyring for the app
  password, encryption optional
date: '2026-11-02T20:01:00Z'
status: accepted
---
## Context

decision-5 makes a folder on the user's own Nextcloud a meeting place for desktop and phone, with sync built into Veritas itself. The spike VRT-38 (`spikes/nextcloud`) answered the open questions. It ran against a local Nextcloud 35.0.2 (the official `nextcloud:apache` image with SQLite, on 127.0.0.1). `e2e.py` checks each step:
- Login Flow v2, with the app password kept only in the keychain.
- A 1-hour Opus recording (14.9 MB) uploaded in chunks: interrupted after one chunk, resumed, and identical on the server (sha256).
- A download interrupted after 3 MB, resumed with a Range request.
- ETag watching that sees a new `transcript.md` appear and the recording move from the inbox to its meeting folder.
- Revoking: the server then answers 401 to the old app password, and the keychain entry is gone.

## Decision

- **HTTP and WebDAV:** raw WebDAV over **ureq 3**, already in the app (decision-2). A PROPFIND/GET/PUT/MKCOL/MOVE/DELETE client plus chunked upload is about 300 lines (`spikes/nextcloud/src/dav.rs`). ureq needs `allow_non_standard_methods(true)` for PROPFIND and MOVE. The PROPFIND answers are parsed with **quick-xml**, a new dependency that brings only `memchr`. No WebDAV crate: those pull in reqwest and a second HTTP stack.
- **Login:** Nextcloud **Login Flow v2**:
  1. POST `/index.php/login/v2`.
  2. Open the login URL in the user's browser.
  3. Poll `/login/v2/poll` every 2 s, for at most 20 minutes.
  
  The app gets an app password the user never sees. Disconnecting revokes it on the server (OCS `DELETE /ocs/v2.php/core/apppassword`) and deletes it locally.
- **Keychain:** the **keyring** crate, version 4, feature `v1`. It uses Secret Service on Linux (through zbus), Keychain on macOS and Credential Manager on Windows. An Android store crate exists for the companion. About 100 crates come with it, mostly zbus. zbus is also what VRT-40 (the global hotkey through the Wayland portal) needs, so it enters the app once for both. The config file holds only the server and login name.
- **Uploads:**
  - Chunked upload v2: MKCOL `uploads/<user>/<transfer>`, PUT numbered chunks of 10 MB, then MOVE `.file` to the destination with `OC-Total-Length`.
  - The transfer id is a hash of path, size and modification time, so an interrupted upload resumes in the same upload folder. The chunks already there (listed with PROPFIND) are skipped.
  - Small files (`transcript.md`, `meta.json`) use a plain PUT.
- **Downloads:** to `<file>.part`, resumed with `Range`. `If-Match` on the ETag makes the server refuse (412) when the file changed in between; the client then starts over.
- **Change detection:** polling. Nextcloud gives a folder a new ETag whenever anything below it changes, so a quiet round costs **one Depth-0 PROPFIND on `Veritas/`**. On a change, the client walks only the folders whose ETag changed (3 PROPFINDs for the test tree). Interval: the server's `pollinterval` capability (60 s by default), plus a check right after the app's own writes and when the app comes to the front. The `notify_push` app can come later as an optional speed-up.
- **Folder layout and ownership:** each file has one writer, so the two apps never edit the same file.

  | Path | Written by | What |
  |---|---|---|
  | `Veritas/Inbox/<name>.opus` + `<name>.meta.json` | phone | a recording, with title, start time and device |
  | `Veritas/<YYYY-MM-DD HH.MM> <title>/` | desktop | the meeting folder: the recording (moved in), `mix.opus`, `transcript.md` |

  After importing an inbox recording, the desktop moves it into the meeting folder with a server-side **MOVE**: no download or upload. The folder name is set once and not renamed when the title changes; `transcript.md` carries the id and the current title.
- **Deleting:**
  - Deleting a meeting on the desktop also removes its folder from Nextcloud, after a confirmation that names Nextcloud.
  - On the phone, removing a transcript only removes the local copy.
  - Meetings sync one way, desktop to Nextcloud. A meeting folder deleted in Nextcloud itself is not deleted on the desktop, and is not uploaded again until the user asks.
- **Encryption:** **optional, off by default.** The server is the user's own. The user asked for transcripts that can be read and searched in Nextcloud itself (decision-5), and files Veritas encrypts would be unreadable there. End-to-end encryption by Veritas (for example age/X25519, with the key passed between desktop and phone by QR code) stays possible later for users whose Nextcloud is hosted by someone else. Voice embeddings and the database never sync, encrypted or not.
- **Tests:** a local container, never a real account. `spikes/nextcloud/nc_grant.py` approves a login flow over HTTP as a browser would. Nextcloud requires the `Origin` header on the login form.

## Consequences

- New crates for VRT-39: `quick-xml` and `keyring` (with zbus).
- AGENTS.md's local-first rule names this connection. It is off by default, it uses an app password from Login Flow v2, and it is revocable.
- Nextcloud's own search finds file names. Full-text search of `transcript.md` in the Nextcloud web UI needs the server's full-text search apps; the companion app searches with its own index (VRT-26.4).
- Not covered by the spike, still open:
  - the phone side: Tauri mobile, the Android keystore, background uploads (VRT-26.3);
  - servers with a self-signed certificate: rustls without the system store refuses them, so certificate pinning or a CA option needs a decision in VRT-39;
  - two-factor accounts: Login Flow v2 handles them in the browser, but this was not tested.

## Settled while building (VRT-39)

- **Certificates:** the sync connection checks HTTPS against the **operating system's trust store** (ureq's `platform-verifier`, i.e. rustls-platform-verifier). A server with a self-signed or private-CA certificate works once the user trusts that certificate on the computer, as for any other program. Veritas has no exceptions or pinning of its own. Plain `http://` is allowed (for a server on the home network), and Settings warns that it is not encrypted.
- **Two-factor accounts** need nothing extra: the second factor is entered in the browser during Login Flow v2. This was not tested against a 2FA account.
- **The sync mode resets on disconnect.** A later connection starts with "Recordings from the phone" and never uploads every meeting to a new server unasked.
- **Folder names use local time.** The Rust side can't safely read the local time zone in a multi-threaded process, so the UI passes its UTC offset at startup, and sync waits for it. Near a daylight-saving change, an old meeting's folder name can be off by an hour; the exact time is in `transcript.md`.
- **Opus imports:** phones record Opus, which symphonia can't decode. The import decodes Opus (in Ogg or WebM) with libopus, which the app already has for the playback file.

