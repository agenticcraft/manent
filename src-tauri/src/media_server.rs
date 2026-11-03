//! Local media server for meeting playback.
//!
//! WebKitGTK can't play media from Tauri's custom protocols, and from blob: URLs it can't
//! seek (Ogg in push mode). Served over HTTP with Range support it gets the duration and
//! seeks exactly. So mix files are served here:
//! - bound to 127.0.0.1 only, on a random port;
//! - every URL carries a random 128-bit token, so other local programs can't guess it;
//! - only `/<token>/meetings/<numeric id>/mix.opus`, and only if that file lies inside the
//!   meetings folder.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::paths::AppPaths;

pub const MIX_FILE: &str = "mix.opus";

pub struct MediaServer {
    base: String,
    server: Arc<Server>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MediaServer {
    pub fn start(paths: AppPaths) -> std::io::Result<Self> {
        let server = Arc::new(
            Server::http("127.0.0.1:0").map_err(|e| std::io::Error::other(e.to_string()))?,
        );
        let port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| std::io::Error::other("no TCP address"))?
            .port();
        let token = random_token()?;
        let base = format!("http://127.0.0.1:{port}/{token}");
        let worker = server.clone();
        let thread = std::thread::Builder::new()
            .name("manent-media".into())
            .spawn(move || {
                // One thread per request: the player keeps its main stream open while it
                // asks for the end of the file (duration) and for seek targets in parallel.
                for request in worker.incoming_requests() {
                    let paths = paths.clone();
                    let token = token.clone();
                    std::thread::spawn(move || {
                        let response = handle(&paths, &token, &request);
                        let _ = request.respond(response);
                    });
                }
            })?;
        Ok(Self {
            base,
            server,
            thread: Some(thread),
        })
    }

    /// URL of a meeting's mix file.
    pub fn mix_url(&self, meeting_id: i64) -> String {
        format!("{}/meetings/{meeting_id}/{MIX_FILE}", self.base)
    }
}

impl Drop for MediaServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn random_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

type Reply = Response<Box<dyn Read + Send>>;

fn reply(status: u16) -> Reply {
    Response::new(
        StatusCode(status),
        vec![],
        Box::new(std::io::empty()),
        Some(0),
        None,
    )
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn handle(paths: &AppPaths, token: &str, request: &tiny_http::Request) -> Reply {
    if !matches!(request.method(), Method::Get | Method::Head) {
        return reply(405);
    }
    let Some(file) = resolve(paths, token, request.url()) else {
        return reply(404);
    };
    let range = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str().to_string());
    serve_file(&file, range.as_deref(), request.method() == &Method::Head)
}

/// Map `/<token>/meetings/<id>/mix.opus` to a file inside the meetings folder.
pub fn resolve(paths: &AppPaths, token: &str, url: &str) -> Option<PathBuf> {
    let path = url.split('?').next()?;
    let mut parts = path.trim_start_matches('/').split('/');
    let (t, kind, id, name) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || t != token || kind != "meetings" || name != MIX_FILE {
        return None;
    }
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let dir = paths.contained_meeting_dir(&paths.meetings_root().join(id))?;
    let file = dir.join(MIX_FILE);
    file.is_file().then_some(file)
}

/// A single byte range `bytes=a-b`, `bytes=a-` or `bytes=-n` against a file of `size` bytes.
/// Returns the inclusive (start, end) or None if unsatisfiable or malformed.
pub fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || size == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => {
            let n: u64 = n.parse().ok()?;
            if n == 0 {
                return None;
            }
            (size.saturating_sub(n), size - 1)
        }
        (s, "") => (s.parse().ok()?, size - 1),
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(size - 1)),
    };
    (start <= end && start < size).then_some((start, end))
}

fn serve_file(path: &Path, range: Option<&str>, head_only: bool) -> Reply {
    let Ok(mut file) = File::open(path) else {
        return reply(404);
    };
    let Ok(size) = file.metadata().map(|m| m.len()) else {
        return reply(500);
    };
    let mut headers = vec![
        header("Content-Type", "audio/ogg"),
        header("Accept-Ranges", "bytes"),
        header("Cache-Control", "no-store"),
    ];
    let (status, start, len) = match range {
        Some(r) => match parse_range(r, size) {
            Some((start, end)) => {
                headers.push(header(
                    "Content-Range",
                    &format!("bytes {start}-{end}/{size}"),
                ));
                (206, start, end - start + 1)
            }
            None => {
                let mut resp = reply(416);
                resp.add_header(header("Content-Range", &format!("bytes */{size}")));
                return resp;
            }
        },
        None => (200, 0, size),
    };
    if head_only {
        return Response::new(
            StatusCode(status),
            headers,
            Box::new(std::io::empty()),
            Some(len as usize),
            None,
        );
    }
    if file.seek(SeekFrom::Start(start)).is_err() {
        return reply(500);
    }
    Response::new(
        StatusCode(status),
        headers,
        Box::new(file.take(len)),
        Some(len as usize),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(
            parse_range("bytes=900-5000", 1000),
            Some((900, 999)),
            "end clamped"
        );
        assert_eq!(parse_range("bytes=1000-", 1000), None, "start past the end");
        assert_eq!(parse_range("bytes=5-1", 1000), None);
        assert_eq!(
            parse_range("bytes=0-1,5-6", 1000),
            None,
            "multiple ranges unsupported"
        );
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=-0", 1000), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
    }

    fn get(url: &str, range: Option<&str>) -> (u16, String, Vec<u8>) {
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let mut s = TcpStream::connect(host).unwrap();
        let range = range.map(|r| format!("Range: {r}\r\n")).unwrap_or_default();
        write!(
            s,
            "GET /{path} HTTP/1.1\r\nHost: {host}\r\n{range}Connection: close\r\n\r\n"
        )
        .unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&buf[..split]).to_string();
        let status = head[9..12].parse().unwrap();
        (status, head, buf[split + 4..].to_vec())
    }

    fn setup() -> (MediaServer, AppPaths, Vec<u8>) {
        let paths = AppPaths::new(crate::test_util::temp_dir());
        let dir = paths.meetings_root().join("7");
        std::fs::create_dir_all(&dir).unwrap();
        let data: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        std::fs::write(dir.join(MIX_FILE), &data).unwrap();
        std::fs::write(paths.data.join("secret.txt"), b"secret").unwrap();
        (MediaServer::start(paths.clone()).unwrap(), paths, data)
    }

    #[test]
    fn serves_whole_files_and_ranges() {
        let (server, _, data) = setup();
        let url = server.mix_url(7);
        assert!(url.starts_with("http://127.0.0.1:"));
        let (status, head, body) = get(&url, None);
        assert_eq!(status, 200);
        assert!(
            head.contains("Accept-Ranges: bytes") && head.contains("audio/ogg"),
            "{head}"
        );
        assert_eq!(body, data);
        let (status, head, body) = get(&url, Some("bytes=100-199"));
        assert_eq!(status, 206);
        assert!(
            head.contains("Content-Range: bytes 100-199/10000"),
            "{head}"
        );
        assert_eq!(body, &data[100..200]);
        let (status, _, body) = get(&url, Some("bytes=-10"));
        assert_eq!((status, body.as_slice()), (206, &data[9990..]));
        let (status, head, _) = get(&url, Some("bytes=20000-"));
        assert_eq!(status, 416);
        assert!(head.contains("bytes */10000"));
    }

    #[test]
    fn a_slow_reader_does_not_block_other_requests() {
        let (server, _, data) = setup();
        let url = server.mix_url(7);
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        // Open a request and don't read the response (like a player buffering).
        let mut slow = TcpStream::connect(host).unwrap();
        write!(slow, "GET /{path} HTTP/1.1\r\nHost: {host}\r\n\r\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let started = std::time::Instant::now();
        let (status, _, body) = get(&url, Some("bytes=-10"));
        assert_eq!((status, body.as_slice()), (206, &data[9990..]));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "not queued behind the slow request"
        );
        drop(slow);
    }

    #[test]
    fn refuses_anything_else() {
        let (server, _, _) = setup();
        let url = server.mix_url(7);
        let base = url.rsplit_once("/meetings/").unwrap().0.to_string();
        let host = base.rsplit_once('/').unwrap().0;
        for bad in [
            format!("{host}/{}/meetings/7/mix.opus", "0".repeat(32)), // wrong token
            format!("{base}/meetings/8/mix.opus"),                    // no such meeting
            format!("{base}/meetings/7/mic.flac"),                    // only the mix
            format!("{base}/meetings/../secret.txt"),
            format!("{base}/meetings/7/../../secret.txt"),
            format!("{base}/meetings/%2e%2e/mix.opus"),
            format!("{base}/meetings/7a/mix.opus"),
            format!("{base}/meetings/7/mix.opus/extra"),
        ] {
            let (status, _, body) = get(&bad, None);
            assert_eq!(status, 404, "{bad}");
            assert!(body.is_empty(), "{bad}");
        }
    }
}
