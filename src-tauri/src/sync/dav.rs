//! A small WebDAV client for Nextcloud over ureq 3 (decision-2, decision-7): PROPFIND with
//! ETags, GET with Range, PUT, MKCOL, MOVE, DELETE, and Nextcloud's chunked upload v2. Grown
//! from the spike in spikes/nextcloud (VRT-38).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::Event;
use ureq::http::Request;
use ureq::{Agent, SendBody};

#[derive(Clone)]
pub struct Account {
    pub server: String,
    pub login: String,
    pub app_password: String,
}

/// One entry of a PROPFIND answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Path below the user's files root, e.g. "Veritas/Inbox/rec.opus" (no leading slash).
    pub path: String,
    pub etag: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Clone)]
pub struct Dav {
    agent: Agent,
    acc: Account,
    auth: String,
}

const PROPFIND: &str = r#"<?xml version="1.0"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop><d:getetag/><d:getcontentlength/><d:resourcetype/><oc:size/></d:prop>
</d:propfind>"#;

/// HTTPS is checked against the operating system's trust store (VRT-39): a server with a
/// certificate the user trusts on this computer (also a self-signed one they added) works,
/// nothing else does.
pub fn agent() -> Agent {
    Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_connect(Some(std::time::Duration::from_secs(20)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(120)))
        .http_status_as_error(false)
        .allow_non_standard_methods(true)
        .user_agent(concat!("Veritas/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Percent-encode a path, keeping the slashes.
pub fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn basic(login: &str, password: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = format!("{login}:{password}").into_bytes();
    let mut s = String::new();
    for c in input.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    format!("Basic {s}")
}

type R<T> = Result<T, String>;

fn bytes(data: &[u8]) -> (SendBody<'static>, Option<usize>) {
    (
        SendBody::from_owned_reader(std::io::Cursor::new(data.to_vec())),
        Some(data.len()),
    )
}

fn none() -> (SendBody<'static>, Option<usize>) {
    (SendBody::none(), None)
}

impl Dav {
    pub fn new(acc: Account) -> Self {
        let auth = basic(&acc.login, &acc.app_password);
        Self {
            agent: agent(),
            acc,
            auth,
        }
    }

    fn files_url(&self, path: &str) -> String {
        format!(
            "{}/remote.php/dav/files/{}/{}",
            self.acc.server,
            encode_path(&self.acc.login),
            encode_path(path.trim_start_matches('/'))
        )
    }

    fn uploads_url(&self, rest: &str) -> String {
        format!(
            "{}/remote.php/dav/uploads/{}/{}",
            self.acc.server,
            encode_path(&self.acc.login),
            rest
        )
    }

    fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, String)],
        body: (SendBody<'static>, Option<usize>),
    ) -> R<ureq::http::Response<ureq::Body>> {
        let mut b = Request::builder()
            .method(method)
            .uri(url)
            .header("Authorization", &self.auth)
            .header("OCS-APIRequest", "true");
        for (k, v) in headers {
            b = b.header(*k, v);
        }
        let (body, len) = body;
        if let Some(len) = len {
            b = b.header("Content-Length", len);
        }
        let req = b.body(body).map_err(|e| e.to_string())?;
        self.agent
            .run(req)
            .map_err(|e| format!("{method} {url}: {e}"))
    }

    fn expect(resp: &mut ureq::http::Response<ureq::Body>, ok: &[u16], what: &str) -> R<()> {
        let code = resp.status().as_u16();
        if ok.contains(&code) {
            Ok(())
        } else {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            Err(format!(
                "{what}: HTTP {code} {}",
                text.chars().take(200).collect::<String>()
            ))
        }
    }

    /// PROPFIND `path` with Depth 0 (itself) or 1 (and its children).
    pub fn propfind(&self, path: &str, depth: u8) -> R<Vec<Entry>> {
        let mut resp = self.send(
            "PROPFIND",
            &self.files_url(path),
            &[
                ("Depth", depth.to_string()),
                ("Content-Type", "application/xml".into()),
            ],
            bytes(PROPFIND.as_bytes()),
        )?;
        Self::expect(&mut resp, &[207], "PROPFIND")?;
        let xml = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let root = format!("/remote.php/dav/files/{}/", self.acc.login);
        parse_multistatus(&xml, &root)
    }

    /// The ETag of one file or folder, or None if it doesn't exist. Nextcloud gives every
    /// folder a new ETag when anything below it changes, so this one call says whether a whole
    /// tree changed.
    pub fn etag(&self, path: &str) -> R<Option<String>> {
        match self.propfind(path, 0) {
            Ok(list) => Ok(list.into_iter().next().map(|e| e.etag)),
            Err(e) if e.contains("HTTP 404") => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn mkcol(&self, path: &str) -> R<()> {
        let mut resp = self.send("MKCOL", &self.files_url(path), &[], none())?;
        // 405: it exists already.
        Self::expect(&mut resp, &[201, 405], "MKCOL")
    }

    pub fn put(&self, path: &str, data: &[u8]) -> R<()> {
        let mut resp = self.send("PUT", &self.files_url(path), &[], bytes(data))?;
        Self::expect(&mut resp, &[201, 204], "PUT")
    }

    /// Move on the server (no download or upload): the inbox recording into its meeting folder.
    pub fn mv(&self, from: &str, to: &str) -> R<()> {
        let headers = [
            ("Destination", self.files_url(to)),
            ("Overwrite", "F".to_string()),
        ];
        let mut resp = self.send("MOVE", &self.files_url(from), &headers, none())?;
        Self::expect(&mut resp, &[201, 204], "MOVE")
    }

    pub fn delete(&self, path: &str) -> R<()> {
        let mut resp = self.send("DELETE", &self.files_url(path), &[], none())?;
        Self::expect(&mut resp, &[204, 404], "DELETE")
    }

    /// Download `path` to `dest`, resuming a `<dest>.part` with a Range request. `If-Match`
    /// makes the server refuse when the file changed since the part was started.
    pub fn download(&self, path: &str, dest: &Path) -> R<(u64, u64)> {
        let part = dest.with_extension(format!(
            "{}.part",
            dest.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        let etag_file = part.with_extension("etag");
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let mut headers = Vec::new();
        if have > 0 {
            headers.push(("Range", format!("bytes={have}-")));
            if let Ok(etag) = std::fs::read_to_string(&etag_file) {
                headers.push(("If-Match", etag));
            }
        }
        let mut resp = self.send("GET", &self.files_url(path), &headers, none())?;
        let code = resp.status().as_u16();
        let (mut out, start) = match code {
            206 => (
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&part)
                    .map_err(|e| e.to_string())?,
                have,
            ),
            200 => (File::create(&part).map_err(|e| e.to_string())?, 0),
            412 => {
                // Changed on the server: start over.
                let _ = std::fs::remove_file(&part);
                let _ = std::fs::remove_file(&etag_file);
                return self.download(path, dest);
            }
            _ => return Err(format!("GET {path}: HTTP {code}")),
        };
        if let Some(etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
            std::fs::write(&etag_file, etag).map_err(|e| e.to_string())?;
        }
        let copied =
            std::io::copy(&mut resp.body_mut().as_reader(), &mut out).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&etag_file);
        Ok((start, copied))
    }

    /// Upload `local` to `path` with Nextcloud's chunked upload v2: chunks go to an upload
    /// folder named after the transfer, then one MOVE assembles them. Chunks already on the
    /// server (an interrupted upload) are skipped. `stop_after` simulates an interruption.
    pub fn upload_chunked(
        &self,
        local: &Path,
        path: &str,
        chunk: u64,
        stop_after: Option<usize>,
        progress: &mut dyn FnMut(String),
    ) -> R<UploadReport> {
        let size = std::fs::metadata(local).map_err(|e| e.to_string())?.len();
        let id = transfer_id(local, path, size)?;
        let dest = self.files_url(path);
        let dir = self.uploads_url(&id);
        let headers = [
            ("Destination", dest.clone()),
            ("OC-Total-Length", size.to_string()),
        ];
        let mut resp = self.send("MKCOL", &dir, &headers, none())?;
        Self::expect(&mut resp, &[201, 405], "MKCOL upload folder")?;
        let present = self.chunks_present(&id)?;
        let mut file = File::open(local).map_err(|e| e.to_string())?;
        let count = size.div_ceil(chunk).max(1);
        let (mut sent, mut skipped) = (0usize, 0usize);
        for n in 1..=count {
            let start = (n - 1) * chunk;
            let len = chunk.min(size - start);
            let name = format!("{n:05}");
            if present.get(&name) == Some(&len) {
                skipped += 1;
                continue;
            }
            if stop_after.is_some_and(|s| sent >= s) {
                return Ok(UploadReport {
                    size,
                    chunks: count as usize,
                    sent,
                    skipped,
                    finished: false,
                });
            }
            let mut buf = vec![0u8; len as usize];
            file.seek(SeekFrom::Start(start))
                .map_err(|e| e.to_string())?;
            file.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let mut resp = self.send("PUT", &format!("{dir}/{name}"), &headers, bytes(&buf))?;
            Self::expect(&mut resp, &[201, 204], "PUT chunk")?;
            sent += 1;
            progress(format!("chunk {n}/{count} sent"));
        }
        let mut resp = self.send("MOVE", &format!("{dir}/.file"), &headers, none())?;
        Self::expect(&mut resp, &[201, 204], "MOVE .file")?;
        Ok(UploadReport {
            size,
            chunks: count as usize,
            sent,
            skipped,
            finished: true,
        })
    }

    /// Chunk name -> size of an upload folder (empty if it doesn't exist yet).
    fn chunks_present(&self, id: &str) -> R<HashMap<String, u64>> {
        let mut resp = self.send(
            "PROPFIND",
            &self.uploads_url(id),
            &[("Depth", "1".into())],
            bytes(PROPFIND.as_bytes()),
        )?;
        if resp.status().as_u16() == 404 {
            return Ok(HashMap::new());
        }
        Self::expect(&mut resp, &[207], "PROPFIND upload folder")?;
        let xml = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let root = format!("/remote.php/dav/uploads/{}/{id}/", self.acc.login);
        Ok(parse_multistatus(&xml, &root)?
            .into_iter()
            .filter(|e| !e.is_dir && !e.path.is_empty())
            .map(|e| (e.path, e.size))
            .collect())
    }

    /// Revoke the app password this client uses (the user disconnects).
    pub fn revoke(&self) -> R<()> {
        let mut resp = self.send(
            "DELETE",
            &format!("{}/ocs/v2.php/core/apppassword", self.acc.server),
            &[],
            none(),
        )?;
        Self::expect(&mut resp, &[200], "revoke app password")
    }
}

/// Login Flow v2, step 1: the page for the browser, and the poll endpoint and token.
pub fn start_login(server: &str) -> R<(String, String, String)> {
    let mut resp = agent()
        .post(format!("{server}/index.php/login/v2"))
        .send_empty()
        .map_err(|e| format!("Can't reach {server}: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!(
            "{server} doesn't look like a Nextcloud server (HTTP {})",
            resp.status()
        ));
    }
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| format!("{server} doesn't look like a Nextcloud server"))?;
    let get = |p: &str| {
        v.pointer(p)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("unexpected login answer from {server}"))
    };
    Ok((get("/login")?, get("/poll/endpoint")?, get("/poll/token")?))
}

/// Login Flow v2, step 2: None until the user approved in the browser, then the account.
pub fn poll_login(endpoint: &str, token: &str) -> R<Option<Account>> {
    let mut resp = agent()
        .post(endpoint)
        .send_form([("token", token)])
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() != 200 {
        return Ok(None);
    }
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    match (
        v["server"].as_str(),
        v["loginName"].as_str(),
        v["appPassword"].as_str(),
    ) {
        (Some(server), Some(login), Some(pw)) => Ok(Some(Account {
            server: server.trim_end_matches('/').to_string(),
            login: login.to_string(),
            app_password: pw.to_string(),
        })),
        _ => Err("unexpected poll answer".into()),
    }
}

#[derive(Debug)]
pub struct UploadReport {
    pub size: u64,
    pub chunks: usize,
    pub sent: usize,
    pub skipped: usize,
    pub finished: bool,
}

/// The same file to the same place resumes the same transfer.
fn transfer_id(local: &Path, path: &str, size: u64) -> R<String> {
    use sha2::{Digest, Sha256};
    let modified = std::fs::metadata(local)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let digest = Sha256::digest(format!("{path}\n{size}\n{modified}").as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("veritas-{}", &hex[..20]))
}

/// The entries of a multistatus answer, with paths below `root` (a URL path prefix).
pub fn parse_multistatus(xml: &str, root: &str) -> R<Vec<Entry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut cur: Option<Entry> = None;
    let mut field = String::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_string();
                if name == "response" {
                    cur = Some(Entry {
                        path: String::new(),
                        etag: String::new(),
                        size: 0,
                        is_dir: false,
                    });
                } else if name == "collection"
                    && let Some(c) = cur.as_mut()
                {
                    c.is_dir = true;
                }
                field = name;
            }
            Event::Empty(e) => {
                if e.local_name().as_ref() == "collection"
                    && let Some(c) = cur.as_mut()
                {
                    c.is_dir = true;
                }
            }
            Event::Text(t) => {
                let text = t.xml10_content().into_owned();
                if let Some(c) = cur.as_mut() {
                    match field.as_str() {
                        "href" => {
                            let p = decode_path(&text);
                            c.path = p
                                .strip_prefix(root)
                                .unwrap_or(&p)
                                .trim_end_matches('/')
                                .to_string();
                        }
                        "getetag" => c.etag = text.trim_matches('"').to_string(),
                        "getcontentlength" | "size" => c.size = text.parse().unwrap_or(c.size),
                        _ => {}
                    }
                }
            }
            Event::End(e) => {
                if e.local_name().as_ref() == "response"
                    && let Some(c) = cur.take()
                {
                    out.push(c);
                }
                field.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_matches_known_values() {
        assert_eq!(basic("vtest", "pw"), "Basic dnRlc3Q6cHc=");
        assert_eq!(basic("a", "b"), "Basic YTpi");
    }

    #[test]
    fn paths_round_trip() {
        let p = "Veritas/Meeting 1 – Q4/transcript.md";
        assert_eq!(decode_path(&encode_path(p)), p);
        assert!(!encode_path(p).contains(' '));
    }

    #[test]
    fn parses_a_nextcloud_multistatus() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
 <d:response><d:href>/remote.php/dav/files/vtest/Veritas/</d:href>
  <d:propstat><d:prop><d:getetag>"abc"</d:getetag><d:resourcetype><d:collection/></d:resourcetype><oc:size>12</oc:size></d:prop></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/vtest/Veritas/Inbox/a%20b.opus</d:href>
  <d:propstat><d:prop><d:getetag>"def"</d:getetag><d:getcontentlength>12</d:getcontentlength><d:resourcetype/></d:prop></d:propstat></d:response>
</d:multistatus>"#;
        let list = parse_multistatus(xml, "/remote.php/dav/files/vtest/").unwrap();
        assert_eq!(
            list[0],
            Entry {
                path: "Veritas".into(),
                etag: "abc".into(),
                size: 12,
                is_dir: true
            }
        );
        assert_eq!(
            list[1],
            Entry {
                path: "Veritas/Inbox/a b.opus".into(),
                etag: "def".into(),
                size: 12,
                is_dir: false
            }
        );
    }
}
