---
id: decision-2
title: Downloads use ureq (blocking HTTP client with rustls)
date: '2026-10-30T09:17:00Z'
status: accepted
---
## Context

VRT-14 downloads the engine (libparakeet, about 14 MB, from the quay.io OCI registry) and the model (1.1 GB, from Hugging Face) at first run. The downloads need HTTPS, redirects (both hosts redirect to a CDN), Range requests for resume, and streaming bodies for progress and hashing. The app has no HTTP client yet. Downloads run on their own thread and the app needs nothing else from HTTP, so an async client brings no benefit.

## Decision

Use `ureq` 3 (blocking, rustls, no OpenSSL) for all downloads. The other crates are:
- `sha2`: verifies every download against a pinned sha256.
- `flate2` and `tar`: unpack the engine image layer.

## Consequences

- There is no system OpenSSL dependency, so the build is the same on Linux, Windows and macOS.
- Download code is plain blocking Rust on a worker thread, and it is tested against a local `tiny_http` server.
- If an async HTTP client is ever needed elsewhere (for example a mirror API), revisit this decision instead of adding a second client.

