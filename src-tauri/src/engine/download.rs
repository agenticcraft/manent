//! Resumable, verified downloads for the engine and the model (VRT-14, decision-2).
//!
//! A file downloads to `<name>.part`. When a download is interrupted (cancel, network
//! error, app quit) the part stays, and the next attempt asks for the rest with a Range
//! request. A file only gets its final name after its size and sha256 match the pin.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::assets::EngineImage;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("download cancelled")]
    Cancelled,
    #[error("{0}: checksum mismatch, the download was discarded")]
    Checksum(String),
    #[error("{0}: expected {1} bytes, got {2}")]
    Size(String, u64, u64),
    #[error("{0}")]
    Http(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl From<ureq::Error> for DownloadError {
    fn from(e: ureq::Error) -> Self {
        Self::Http(e.to_string())
    }
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        // 4xx/5xx are handled here (a 416 means the part file is unusable).
        .http_status_as_error(false)
        .user_agent(concat!("veritas/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Bytes already downloaded for `dest` (0 when there is no part file).
pub fn partial_len(dest: &Path) -> u64 {
    std::fs::metadata(part_path(dest)).map_or(0, |m| m.len())
}

/// Download `url` to `dest` and check it against `size` and `sha256` (lowercase hex).
/// Resumes a previous part file. `progress` gets the bytes received so far (including the
/// resumed part); `cancel` stops the download and keeps the part.
pub fn download(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    size: u64,
    sha256: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), DownloadError> {
    let name = dest
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(dest);
    let mut hasher = Sha256::new();
    let mut have = 0u64;
    if let Ok(mut f) = File::open(&part) {
        // Hash what is already there, so the final check covers the whole file.
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            have += n as u64;
        }
    }
    if have > size {
        std::fs::remove_file(&part)?;
        hasher = Sha256::new();
        have = 0;
    }
    progress(have);

    if have < size {
        let mut req = agent.get(url).header("Accept-Encoding", "identity");
        if have > 0 {
            req = req.header("Range", &format!("bytes={have}-"));
        }
        let resp = req.call()?;
        let status = resp.status().as_u16();
        let mut file = match status {
            206 if have > 0 => OpenOptions::new().append(true).open(&part)?,
            200 => {
                // The server sent the whole file: start over.
                hasher = Sha256::new();
                have = 0;
                File::create(&part)?
            }
            416 => {
                std::fs::remove_file(&part)?;
                return Err(DownloadError::Http(format!(
                    "{name}: the server refused to resume; try again"
                )));
            }
            _ => return Err(DownloadError::Http(format!("{name}: HTTP {status}"))),
        };
        let mut body = resp
            .into_body()
            .into_with_config()
            // ureq fails a body that reaches the limit exactly; the size check below is strict.
            .limit(size - have + 1)
            .reader();
        let mut buf = vec![0u8; 256 << 10];
        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush()?;
                return Err(DownloadError::Cancelled);
            }
            let n = match body.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    file.flush()?;
                    return Err(DownloadError::Http(format!("{name}: {e}")));
                }
            };
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            have += n as u64;
            progress(have);
        }
        file.sync_all()?;
    }
    if have != size {
        return Err(DownloadError::Size(name, size, have));
    }
    if hex(&hasher.finalize()) != sha256 {
        std::fs::remove_file(&part)?;
        return Err(DownloadError::Checksum(name));
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// The layer (digest, size) of a single-platform image manifest whose bytes hash to `digest`.
fn image_layer(agent: &ureq::Agent, image: &EngineImage) -> Result<(String, u64), DownloadError> {
    let url = format!(
        "{}/v2/{}/manifests/{}",
        image.registry, image.repository, image.manifest
    );
    let resp = agent
        .get(&url)
        .header(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json",
        )
        .call()?;
    if resp.status().as_u16() != 200 {
        return Err(DownloadError::Http(format!(
            "engine manifest: HTTP {}",
            resp.status()
        )));
    }
    let mut raw = Vec::new();
    resp.into_body()
        .into_with_config()
        .limit(1 << 20)
        .reader()
        .read_to_end(&mut raw)?;
    if format!("sha256:{}", hex(&Sha256::digest(&raw))) != image.manifest {
        return Err(DownloadError::Checksum("engine manifest".into()));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|e| DownloadError::Other(e.to_string()))?;
    let layers = manifest["layers"].as_array().cloned().unwrap_or_default();
    let [layer] = layers.as_slice() else {
        return Err(DownloadError::Other(format!(
            "engine image has {} layers, expected 1",
            layers.len()
        )));
    };
    let digest = layer["digest"].as_str().unwrap_or_default().to_string();
    let size = layer["size"].as_u64().unwrap_or_default();
    if !digest.starts_with("sha256:") || size == 0 {
        return Err(DownloadError::Other("engine image: bad layer entry".into()));
    }
    Ok((digest, size))
}

/// sha256 (lowercase hex) of a file, read in 1 MB blocks.
pub fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buf[..n]);
    }
}

/// Files of the installed engine that are missing or changed since installation (by the
/// hashes in engine.json; an install without hashes is only checked for presence).
pub fn damaged_engine_files(dir: &Path, image: &EngineImage) -> Vec<String> {
    let recorded: serde_json::Value = std::fs::read_to_string(dir.join(INSTALLED))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    image
        .files
        .iter()
        .filter(|(_, name)| {
            let path = dir.join(name);
            match (recorded["files"][*name].as_str(), file_sha256(&path)) {
                (_, Err(_)) => true,
                (Some(expected), Ok(actual)) => expected != actual,
                (None, Ok(_)) => false,
            }
        })
        .map(|(_, name)| name.to_string())
        .collect()
}

/// File that records which image the engine folder holds.
const INSTALLED: &str = "engine.json";

/// Whether `dir` holds the files of exactly this pinned image.
pub fn engine_installed(dir: &Path, image: &EngineImage) -> bool {
    let recorded = std::fs::read_to_string(dir.join(INSTALLED))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["manifest"].as_str().map(str::to_string));
    recorded.as_deref() == Some(image.manifest)
        && image.files.iter().all(|(_, name)| dir.join(name).is_file())
}

/// Download the engine image layer (resumable, hash-checked) and keep only `image.files`.
/// `progress` gets (received, total) bytes of the layer.
pub fn install_engine(
    agent: &ureq::Agent,
    image: &EngineImage,
    dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), DownloadError> {
    let (digest, size) = image_layer(agent, image)?;
    progress(0, size);
    let layer_file = dir.join(format!("layer-{}.tar.gz", &digest[7..19]));
    let url = format!(
        "{}/v2/{}/blobs/{}",
        image.registry, image.repository, digest
    );
    download(
        agent,
        &url,
        &layer_file,
        size,
        &digest[7..],
        cancel,
        &mut |got| progress(got, size),
    )?;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(&layer_file)?));
    let mut found = Vec::new();
    let mut hashes = serde_json::Map::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry
            .path()?
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();
        if let Some((_, name)) = image.files.iter().find(|(p, _)| *p == path) {
            if !entry.header().entry_type().is_file() {
                return Err(DownloadError::Other(format!(
                    "{path} is not a regular file"
                )));
            }
            let tmp = dir.join(format!("{name}.tmp"));
            let mut out = File::create(&tmp)?;
            std::io::copy(&mut entry, &mut out)?;
            out.sync_all()?;
            std::fs::rename(&tmp, dir.join(name))?;
            found.push(*name);
            hashes.insert(name.to_string(), file_sha256(&dir.join(name))?.into());
        }
    }
    if let Some((path, _)) = image.files.iter().find(|(_, n)| !found.contains(n)) {
        return Err(DownloadError::Other(format!("engine image has no {path}")));
    }
    std::fs::write(
        dir.join(INSTALLED),
        // The file hashes let "Check" (VRT-27) find a damaged library later.
        serde_json::json!({ "manifest": image.manifest, "layer": digest, "files": hashes })
            .to_string(),
    )?;
    std::fs::remove_file(&layer_file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    /// A local server for one file, honouring `Range: bytes=N-`. Can stop sending after
    /// `cut` bytes (to simulate a dropped connection) and counts requests.
    struct FileServer {
        url: String,
        requests: Arc<AtomicUsize>,
        _server: Arc<tiny_http::Server>,
    }

    fn serve(data: Vec<u8>, cut: Option<usize>, honour_range: bool) -> FileServer {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let requests = Arc::new(AtomicUsize::new(0));
        let (srv, count) = (server.clone(), requests.clone());
        std::thread::spawn(move || {
            for req in srv.incoming_requests() {
                let n = count.fetch_add(1, Ordering::SeqCst);
                let start = req
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Range"))
                    .and_then(|h| {
                        h.value
                            .as_str()
                            .strip_prefix("bytes=")?
                            .strip_suffix('-')?
                            .parse()
                            .ok()
                    })
                    .filter(|_| honour_range);
                let (status, body) = match start {
                    Some(s) => (206, data[s..].to_vec()),
                    None => (200, data.clone()),
                };
                // Only the first request is cut short.
                let body = match cut {
                    Some(c) if n == 0 => body[..c.min(body.len())].to_vec(),
                    _ => body,
                };
                let _ = req.respond(tiny_http::Response::new(
                    tiny_http::StatusCode(status),
                    vec![],
                    std::io::Cursor::new(body.clone()),
                    Some(body.len()),
                    None,
                ));
            }
        });
        FileServer {
            url: format!("http://127.0.0.1:{port}/file"),
            requests,
            _server: server,
        }
    }

    fn data() -> (Vec<u8>, String) {
        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let sha = hex(&Sha256::digest(&data));
        (data, sha)
    }

    #[test]
    fn downloads_and_verifies() {
        let (data, sha) = data();
        let server = serve(data.clone(), None, true);
        let dest = crate::test_util::temp_dir().join("model.gguf");
        let mut last = 0;
        download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &AtomicBool::new(false),
            &mut |n| last = n,
        )
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(last, data.len() as u64);
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn resumes_a_part_file_with_a_range_request() {
        let (data, sha) = data();
        let dest = crate::test_util::temp_dir().join("model.gguf");
        std::fs::write(part_path(&dest), &data[..120_000]).unwrap();
        let server = serve(data.clone(), None, true);
        let mut first = None;
        download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &AtomicBool::new(false),
            &mut |n| {
                first.get_or_insert(n);
            },
        )
        .unwrap();
        assert_eq!(first, Some(120_000), "progress starts at the resumed size");
        assert_eq!(std::fs::read(&dest).unwrap(), data);
    }

    #[test]
    fn a_dropped_connection_keeps_the_part_and_the_retry_completes_it() {
        let (data, sha) = data();
        let dest = crate::test_util::temp_dir().join("model.gguf");
        let server = serve(data.clone(), Some(100_000), true);
        let err = download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &AtomicBool::new(false),
            &mut |_| {},
        );
        assert!(err.is_err(), "short body is an error");
        assert_eq!(partial_len(&dest), 100_000);
        download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_server_without_range_support_restarts_cleanly() {
        let (data, sha) = data();
        let dest = crate::test_util::temp_dir().join("model.gguf");
        std::fs::write(part_path(&dest), &data[..50_000]).unwrap();
        let server = serve(data.clone(), None, false);
        download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
    }

    #[test]
    fn a_wrong_checksum_discards_the_file() {
        let (data, _) = data();
        let dest = crate::test_util::temp_dir().join("model.gguf");
        let server = serve(data.clone(), None, true);
        let err = download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &"0".repeat(64),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        assert!(matches!(err, Err(DownloadError::Checksum(_))), "{err:?}");
        assert!(!dest.exists() && !part_path(&dest).exists());
    }

    #[test]
    fn cancel_keeps_the_part() {
        let (data, sha) = data();
        let dest = crate::test_util::temp_dir().join("model.gguf");
        let server = serve(data.clone(), None, true);
        let cancel = AtomicBool::new(false);
        let err = download(
            &agent(),
            &server.url,
            &dest,
            data.len() as u64,
            &sha,
            &cancel,
            &mut |n| {
                if n > 0 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert!(matches!(err, Err(DownloadError::Cancelled)), "{err:?}");
        assert!(partial_len(&dest) > 0 && !dest.exists());
    }

    #[test]
    fn installs_only_the_pinned_files_from_an_image() {
        // Build a gzip'd tar layer and a manifest that names it, served like a registry.
        let mut tar_bytes = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_bytes);
            for (path, body) in [
                ("lib/libparakeet.so", &b"engine"[..]),
                ("lib/libc.so.6", b"glibc"),
                ("run.sh", b"#!"),
            ] {
                let mut h = tar::Header::new_gnu();
                h.set_size(body.len() as u64);
                h.set_mode(0o755);
                h.set_cksum();
                b.append_data(&mut h, path, body).unwrap();
            }
            b.finish().unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar_bytes).unwrap();
        let layer = gz.finish().unwrap();
        let layer_digest = format!("sha256:{}", hex(&Sha256::digest(&layer)));
        let manifest =
            serde_json::json!({"layers": [{"digest": layer_digest, "size": layer.len()}]})
                .to_string();
        let manifest_digest: &'static str = Box::leak(
            format!("sha256:{}", hex(&Sha256::digest(manifest.as_bytes()))).into_boxed_str(),
        );

        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let (srv, layer_c, manifest_c) = (server.clone(), layer.clone(), manifest.clone());
        std::thread::spawn(move || {
            for req in srv.incoming_requests() {
                let body = if req.url().contains("/manifests/") {
                    manifest_c.clone().into_bytes()
                } else {
                    layer_c.clone()
                };
                let _ = req.respond(tiny_http::Response::from_data(body));
            }
        });
        let registry: &'static str = Box::leak(format!("http://127.0.0.1:{port}").into_boxed_str());
        let image = EngineImage {
            registry,
            repository: "test/repo",
            manifest: manifest_digest,
            files: &[("lib/libparakeet.so", "libparakeet.so")],
            library: "libparakeet.so",
        };
        let dir = crate::test_util::temp_dir();
        assert!(!engine_installed(&dir, &image));
        install_engine(
            &agent(),
            &image,
            &dir,
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
        .unwrap();
        assert!(engine_installed(&dir, &image));
        assert_eq!(
            std::fs::read(dir.join("libparakeet.so")).unwrap(),
            b"engine"
        );
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["engine.json", "libparakeet.so"],
            "no glibc, no layer left behind"
        );
        assert!(damaged_engine_files(&dir, &image).is_empty());
        std::fs::write(dir.join("libparakeet.so"), b"changed").unwrap();
        assert_eq!(damaged_engine_files(&dir, &image), ["libparakeet.so"]);
        std::fs::remove_file(dir.join("libparakeet.so")).unwrap();
        assert_eq!(damaged_engine_files(&dir, &image), ["libparakeet.so"]);
    }
}
