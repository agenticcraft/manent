//! Client side of the `veritas-engine` helper process (protocol in engine-helper/src/main.rs).
//!
//! One reader thread routes answers to waiting requests by id. When the process ends
//! (crash, kill, out of memory) every waiting request fails with "engine stopped" and the
//! owner is told through `on_exit`; the app itself is never affected.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::oneshot;

/// The helper's ready line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyInfo {
    pub abi: u32,
    pub load_ms: u64,
    pub backends: i32,
    pub components: Vec<String>,
}

pub struct Launch {
    pub exe: PathBuf,
    pub library: PathBuf,
    pub model: PathBuf,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct Helper {
    child: Arc<Mutex<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    stopping: Arc<AtomicBool>,
    pub info: ReadyInfo,
    pub pid: u32,
}

/// Loading the model takes under a second on the test machine; allow for slow disks.
const READY_TIMEOUT: Duration = Duration::from_secs(120);
const STDERR_LINES: usize = 20;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The helper binary next to the app binary (in tests: next to `target/<profile>/deps`).
pub fn helper_exe() -> Option<PathBuf> {
    let name = format!("veritas-engine{}", std::env::consts::EXE_SUFFIX);
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join(&name), dir.parent()?.join(&name)]
        .into_iter()
        .find(|p| p.is_file())
}

/// Environment so the helper finds the libraries next to libparakeet (OpenMP runtime).
fn library_path_var() -> &'static str {
    if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

impl Helper {
    /// Start the helper and wait until the model is loaded. `on_exit` runs once, on the
    /// reader thread, when the process ends without `stop()` (the reason is human-readable).
    pub fn spawn(
        launch: &Launch,
        on_exit: impl FnOnce(String) + Send + 'static,
    ) -> Result<Arc<Self>, String> {
        let lib_dir = launch.library.parent().unwrap_or(Path::new("."));
        let mut child = Command::new(&launch.exe)
            .arg("--lib")
            .arg(&launch.library)
            .arg("--model")
            .arg(&launch.model)
            .env(library_path_var(), lib_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("starting {}: {e}", launch.exe.display()))?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = child.stderr.take().ok_or("no stderr")?;
        let child = Arc::new(Mutex::new(child));

        // libparakeet logs to stderr: forward it to the app log, keep the tail for errors.
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let tail_w = tail.clone();
        std::thread::Builder::new()
            .name("veritas-engine-stderr".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    eprintln!("[engine] {line}");
                    let mut t = lock(&tail_w);
                    if t.len() == STDERR_LINES {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            })
            .map_err(|e| e.to_string())?;

        let pending: Pending = Arc::default();
        let stopping = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<String>();
        {
            let (pending, stopping, child, tail) = (
                pending.clone(),
                stopping.clone(),
                child.clone(),
                tail.clone(),
            );
            std::thread::Builder::new()
                .name("veritas-engine-reader".into())
                .spawn(move || {
                    let mut lines = BufReader::new(stdout).lines().map_while(Result::ok);
                    if let Some(first) = lines.next() {
                        let _ = ready_tx.send(first);
                    }
                    drop(ready_tx);
                    for line in lines {
                        route(&pending, &line);
                    }
                    // stdout closed: the process is ending.
                    for (_, tx) in lock(&pending).drain() {
                        let _ = tx.send(Err("engine stopped".into()));
                    }
                    let status = lock(&child).wait();
                    if !stopping.load(Ordering::SeqCst) {
                        let last = lock(&tail).back().cloned().unwrap_or_default();
                        let status = status.map_or_else(|e| e.to_string(), |s| s.to_string());
                        on_exit(if last.is_empty() {
                            status
                        } else {
                            format!("{status} ({last})")
                        });
                    }
                })
                .map_err(|e| e.to_string())?;
        }

        let fail = |msg: String| {
            stopping.store(true, Ordering::SeqCst);
            let _ = lock(&child).kill();
            Err(msg)
        };
        let first = match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(line) => line,
            Err(_) => {
                // Give the stderr thread a moment to collect the reason.
                std::thread::sleep(Duration::from_millis(100));
                let why = lock(&tail).iter().cloned().collect::<Vec<_>>().join(" / ");
                return fail(format!("The engine did not start: {why}"));
            }
        };
        let msg: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
        if msg["event"] != "ready" {
            let err = msg["error"].as_str().unwrap_or(&first).to_string();
            return fail(format!("The engine did not start: {err}"));
        }
        let info: ReadyInfo = match serde_json::from_value(msg) {
            Ok(i) => i,
            Err(e) => return fail(format!("The engine ready line: {e}")),
        };
        Ok(Arc::new(Self {
            child,
            stdin: Mutex::new(stdin),
            pending,
            next_id: AtomicU64::new(1),
            stopping,
            info,
            pid,
        }))
    }

    /// Send one request and wait for its answer.
    pub async fn request(&self, op: &str, wav: Option<&Path>) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        let line = json!({ "id": id, "op": op, "wav": wav });
        let sent = match lock(&self.stdin).as_mut() {
            Some(stdin) => writeln!(stdin, "{line}")
                .and_then(|_| stdin.flush())
                .is_ok(),
            None => false,
        };
        if !sent {
            lock(&self.pending).remove(&id);
            return Err("engine stopped".into());
        }
        rx.await.unwrap_or_else(|_| Err("engine stopped".into()))
    }

    /// Stop on purpose (app exit, engine update): no `on_exit`, no restart.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        // Closing stdin lets the helper exit by itself; kill makes sure.
        lock(&self.stdin).take();
        let _ = lock(&self.child).kill();
    }
}

fn route(pending: &Pending, line: &str) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        eprintln!("[engine] unexpected output: {line}");
        return;
    };
    let Some(id) = msg["id"].as_u64() else {
        eprintln!("[engine] {line}");
        return;
    };
    let Some(tx) = lock(pending).remove(&id) else {
        return;
    };
    let reply = match msg.get("error").and_then(Value::as_str) {
        Some(err) => Err(err.to_string()),
        None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
    };
    let _ = tx.send(reply);
}
