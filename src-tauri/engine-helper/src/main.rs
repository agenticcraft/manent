//! veritas-engine: runs libparakeet (parakeet.cpp's C API, ABI v10) in its own process, so
//! an engine crash or out-of-memory never takes a recording down (decision-1, doc-2 §1.3).
//!
//! Usage: veritas-engine --lib <libparakeet.so> --model <parakeet-bundle-standard.gguf>
//!
//! Protocol: JSON lines. On start the helper loads the model and prints one line,
//! `{"event":"ready","abi":10,"components":[..],"load_ms":..,"backends":..}` or
//! `{"event":"failed","error":".."}` (then exits with code 2). After that it reads requests
//! from stdin, one per line, and answers each on stdout with the same id:
//! - `{"id":1,"op":"transcribe","wav":"/path.wav"}` -> `{"id":1,"result":{"text","words":[{"w","start","end","conf"}]}}`
//! - `{"id":2,"op":"diarize","wav":"/path.wav"}` -> `{"id":2,"result":{"segments",..,"speaker_profiles"}}`
//! - `{"id":3,"op":"ping"}` -> `{"id":3,"result":{}}`
//!
//! Failures answer `{"id":..,"error":".."}`. Requests run in parallel, one thread each.
//! The helper exits when stdin closes (the app quit or crashed).

mod parakeet;

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Request {
    id: u64,
    op: String,
    #[serde(default)]
    wav: Option<PathBuf>,
}

fn arg(args: &[String], name: &str) -> Option<PathBuf> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).map(PathBuf::from)
}

fn emit(out: &Mutex<std::io::Stdout>, line: &Value) {
    let mut out = out.lock().unwrap_or_else(|e| e.into_inner());
    // If the app is gone, writing fails and the stdin loop ends the process anyway.
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn handle(engine: &parakeet::Engine, req: &Request) -> Result<Value, String> {
    match req.op.as_str() {
        "ping" => Ok(json!({})),
        "transcribe" => {
            let wav = req.wav.as_deref().ok_or("transcribe needs \"wav\"")?;
            let mut doc = engine.transcribe(wav)?;
            // Token ids are not used by the app; words carry the timestamps.
            if let Some(obj) = doc.as_object_mut() {
                obj.remove("tokens");
            }
            Ok(doc)
        }
        "diarize" => {
            let wav = req.wav.as_deref().ok_or("diarize needs \"wav\"")?;
            let (samples, rate) = read_wav_mono(wav)?;
            engine.diarize(&samples, rate)
        }
        other => Err(format!("unknown op {other:?}")),
    }
}

/// A WAV file as mono f32 samples and its sample rate.
fn read_wav_mono(path: &std::path::Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
    }
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let ch = usize::from(spec.channels.max(1));
    let mono = if ch == 1 {
        samples
    } else {
        samples
            .chunks(ch)
            .map(|c| c.iter().sum::<f32>() / ch as f32)
            .collect()
    };
    Ok((mono, spec.sample_rate))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let (Some(lib), Some(model)) = (arg(&args, "--lib"), arg(&args, "--model")) else {
        emit(
            &out,
            &json!({"event": "failed", "error": "usage: veritas-engine --lib <libparakeet> --model <bundle.gguf>"}),
        );
        std::process::exit(2);
    };
    let started = Instant::now();
    let engine = match parakeet::Engine::load(&lib, &model) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            emit(&out, &json!({"event": "failed", "error": e}));
            std::process::exit(2);
        }
    };
    emit(
        &out,
        &json!({
            "event": "ready",
            "abi": engine.abi,
            "components": parakeet::COMPONENTS,
            "load_ms": started.elapsed().as_millis() as u64,
            "backends": engine.backends,
        }),
    );

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                emit(
                    &out,
                    &json!({"id": null, "error": format!("bad request: {e}")}),
                );
                continue;
            }
        };
        let (engine, out) = (engine.clone(), out.clone());
        std::thread::spawn(move || {
            let reply = match handle(&engine, &req) {
                Ok(result) => json!({"id": req.id, "result": result}),
                Err(error) => json!({"id": req.id, "error": error}),
            };
            emit(&out, &reply);
        });
    }
    // stdin closed: the app is gone. Don't wait for running requests.
    std::process::exit(0);
}
