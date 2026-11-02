//! VRT-38 spike: Nextcloud sync built into Veritas, over WebDAV, without the Nextcloud
//! client apps (decision-5).
//!
//! nc-spike login <server> [--no-browser]     Login Flow v2; app password into the keychain
//! nc-spike mkdir <path>
//! nc-spike upload <local> <remote> [--chunk-mb N] [--stop-after N]
//! nc-spike download <remote> <local>
//! nc-spike put <remote> <text>
//! nc-spike rm <remote>
//! nc-spike mv <remote> <remote>
//! nc-spike etag <path>
//! nc-spike watch <dir> [--interval S] [--rounds N]  report what changed below <dir>
//! nc-spike revoke                             revoke the app password, forget it locally
//!
//! The account (server and login name, not the password) is kept in $NC_SPIKE_STATE
//! (default ./nc-spike.json); the app password only in the OS keychain.

mod dav;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use dav::{Account, Dav};

const KEYCHAIN_SERVICE: &str = "veritas-spike-nextcloud";

type R<T> = Result<T, String>;

fn state_file() -> PathBuf {
    std::env::var_os("NC_SPIKE_STATE").map_or_else(|| PathBuf::from("nc-spike.json"), PathBuf::from)
}

fn keychain(server: &str, login: &str) -> R<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, &format!("{login}@{server}")).map_err(|e| e.to_string())
}

fn account() -> R<Account> {
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(state_file())
            .map_err(|_| "not logged in (run login)".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let server = state["server"].as_str().ok_or("bad state")?.to_string();
    let login = state["login"].as_str().ok_or("bad state")?.to_string();
    let app_password = keychain(&server, &login)?
        .get_password()
        .map_err(|e| format!("app password not in the keychain: {e}"))?;
    Ok(Account {
        server,
        login,
        app_password,
    })
}

/// Login Flow v2: ask the server for a flow, let the user approve it in the browser, poll for
/// the app password. The password never passes through the app's UI.
fn login(server: &str, open_browser: bool) -> R<()> {
    let server = server.trim_end_matches('/');
    let agent = dav::agent();
    let mut resp = agent
        .post(format!("{server}/index.php/login/v2"))
        .header("User-Agent", "Veritas (spike)")
        .send_empty()
        .map_err(|e| e.to_string())?;
    let flow: Value = serde_json::from_str(
        &resp
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("not a Nextcloud server? {e}"))?;
    let url = flow["login"].as_str().ok_or("no login url")?;
    let (endpoint, token) = (
        flow["poll"]["endpoint"]
            .as_str()
            .ok_or("no poll endpoint")?,
        flow["poll"]["token"].as_str().ok_or("no poll token")?,
    );
    println!("LOGIN_URL {url}");
    if open_browser {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    // The flow is valid for 20 minutes; poll every 2 s.
    let started = Instant::now();
    loop {
        if started.elapsed() > Duration::from_secs(20 * 60) {
            return Err("login timed out".into());
        }
        std::thread::sleep(Duration::from_secs(2));
        let mut resp = agent
            .post(endpoint)
            .send_form([("token", token)])
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            continue; // 404 until the user approved
        }
        let got: Value = serde_json::from_str(
            &resp
                .body_mut()
                .read_to_string()
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let (server, login, password) = (
            got["server"].as_str().ok_or("no server")?,
            got["loginName"].as_str().ok_or("no loginName")?,
            got["appPassword"].as_str().ok_or("no appPassword")?,
        );
        keychain(server, login)?
            .set_password(password)
            .map_err(|e| format!("keychain: {e}"))?;
        std::fs::write(
            state_file(),
            json!({"server": server, "login": login}).to_string(),
        )
        .map_err(|e| e.to_string())?;
        println!("logged in as {login} on {server}; app password stored in the keychain");
        return Ok(());
    }
}

/// Everything below `dir` with its ETag, walking only into folders whose ETag changed since
/// `known` (Nextcloud propagates ETags upwards, so unchanged folders are skipped whole).
/// Path -> (ETag, is a folder).
type Tree = BTreeMap<String, (String, bool)>;

fn scan(d: &Dav, dir: &str, known: &Tree, out: &mut Tree) -> R<usize> {
    let mut calls = 1;
    for e in d.propfind(dir, 1)? {
        out.insert(e.path.clone(), (e.etag.clone(), e.is_dir));
        if e.is_dir && e.path != dir.trim_end_matches('/') {
            if known.get(&e.path).is_some_and(|(etag, _)| *etag == e.etag) {
                // Unchanged folder: keep what we knew below it.
                for (k, v) in known.range(format!("{}/", e.path)..) {
                    if !k.starts_with(&format!("{}/", e.path)) {
                        break;
                    }
                    out.insert(k.clone(), v.clone());
                }
            } else {
                calls += scan(d, &e.path, known, out)?;
            }
        }
    }
    Ok(calls)
}

fn watch(d: &Dav, dir: &str, interval: u64, rounds: usize) -> R<()> {
    let mut known = BTreeMap::new();
    let calls = scan(d, dir, &BTreeMap::new(), &mut known)?;
    let mut root = known
        .get(dir.trim_end_matches('/'))
        .map(|(etag, _)| etag.clone());
    println!("WATCH {} entries ({calls} PROPFIND calls)", known.len());
    for _ in 0..rounds {
        std::thread::sleep(Duration::from_secs(interval));
        // One Depth-0 PROPFIND per round when nothing changed.
        let now = d.etag(dir)?;
        if now == root {
            println!("UNCHANGED");
            continue;
        }
        let mut fresh = BTreeMap::new();
        let calls = scan(d, dir, &known, &mut fresh)?;
        for (path, (etag, folder)) in &fresh {
            match known.get(path) {
                None => println!("ADDED {path}"),
                Some((old, _)) if old != etag && !folder => println!("CHANGED {path}"),
                _ => {}
            }
        }
        for path in known.keys().filter(|p| !fresh.contains_key(*p)) {
            println!("REMOVED {path}");
        }
        println!("SCANNED with {calls} PROPFIND calls");
        known = fresh;
        root = now;
    }
    Ok(())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String]) -> R<()> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let arg = |i: usize| {
        args.get(i)
            .cloned()
            .ok_or_else(|| format!("{cmd}: missing argument"))
    };
    match cmd {
        "login" => login(&arg(1)?, !args.iter().any(|a| a == "--no-browser")),
        "mkdir" => Dav::new(account()?).mkcol(&arg(1)?),
        "put" => Dav::new(account()?).put(&arg(1)?, arg(2)?.as_bytes()),
        "rm" => Dav::new(account()?).delete(&arg(1)?),
        "mv" => Dav::new(account()?).mv(&arg(1)?, &arg(2)?),
        "etag" => {
            println!(
                "{}",
                Dav::new(account()?)
                    .etag(&arg(1)?)?
                    .unwrap_or_else(|| "-".into())
            );
            Ok(())
        }
        "upload" => {
            let chunk = flag(args, "--chunk-mb")
                .map_or(Ok(10), |v| v.parse::<u64>())
                .map_err(|e| e.to_string())?;
            let stop = flag(args, "--stop-after")
                .map(|v| v.parse::<usize>())
                .transpose()
                .map_err(|e| e.to_string())?;
            let started = Instant::now();
            let r = Dav::new(account()?).upload_chunked(
                Path::new(&arg(1)?),
                &arg(2)?,
                chunk * 1024 * 1024,
                stop,
                &mut |m| eprintln!("{m}"),
            )?;
            println!(
                "UPLOAD size={} chunks={} sent={} skipped={} finished={} in {:.1} s",
                r.size,
                r.chunks,
                r.sent,
                r.skipped,
                r.finished,
                started.elapsed().as_secs_f64()
            );
            Ok(())
        }
        "download" => {
            let (from, n) = Dav::new(account()?).download(&arg(1)?, Path::new(&arg(2)?))?;
            println!(
                "DOWNLOAD {} bytes (resumed at {from}, fetched {n})",
                from + n
            );
            Ok(())
        }
        "watch" => {
            let interval = flag(args, "--interval")
                .map_or(Ok(2), |v| v.parse())
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
            let rounds = flag(args, "--rounds")
                .map_or(Ok(3), |v| v.parse())
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
            watch(&Dav::new(account()?), &arg(1)?, interval, rounds)
        }
        "revoke" => {
            let acc = account()?;
            let entry = keychain(&acc.server, &acc.login)?;
            Dav::new(acc).revoke()?;
            entry.delete_credential().map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(state_file());
            println!("REVOKED; app password removed from the server and the keychain");
            Ok(())
        }
        _ => Err("usage: see the top of src/main.rs".into()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
