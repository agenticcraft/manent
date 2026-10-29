//! VRT-1 spike: capture the microphone and the system output (sink monitor)
//! at the same time on Linux/PipeWire, each into its own WAV file.
//!
//! Usage:
//!   audio-capture list
//!   audio-capture record [seconds] [--mic <source>] [--system <source>] [--out <dir>]
//!   audio-capture record-cpal [seconds] [--out <dir>]
//!
//! `record` uses libpulse-simple: sources are PulseAudio source names (served
//! by pipewire-pulse). Defaults are the special names @DEFAULT_SOURCE@ (default
//! mic) and @DEFAULT_MONITOR@ (monitor of the default output sink).
//!
//! `record-cpal` uses cpal's PipeWire host: the default input device for the
//! mic, and an *input* stream on the default output device for system audio
//! (cpal sets stream.capture.sink, i.e. loopback). The audio callback only
//! pushes into an rtrb ring buffer; a writer thread drains it to WAV.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use libpulse_binding as pulse;
use libpulse_simple_binding::Simple;
use pulse::callbacks::ListResult;
use pulse::context::{Context, FlagSet as ContextFlags, State as ContextState};
use pulse::def::BufferAttr;
use pulse::mainloop::standard::{IterateResult, Mainloop};
use pulse::operation::State as OpState;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;

const RATE: u32 = 48_000;
const CHANNELS: u8 = 2;
/// ~20 ms of interleaved f32 frames per read.
const CHUNK_FRAMES: usize = (RATE as usize) / 50;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("list") => list(),
        Some("record") => record(&args[1..]),
        Some("record-cpal") => record_cpal(&args[1..]),
        _ => {
            eprintln!(
                "usage: audio-capture list | record [seconds] [--mic SRC] [--system SRC] [--out DIR] | record-cpal [seconds] [--out DIR]"
            );
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- list

struct SourceRow {
    name: String,
    description: String,
    is_monitor: bool,
    spec: String,
}

fn list() -> Result<()> {
    let (defaults, sources) = pulse_sources()?;
    println!("PulseAudio API (pipewire-pulse)");
    println!("  default source: {}", defaults.0);
    println!(
        "  default sink:   {} (monitor: {}.monitor)",
        defaults.1, defaults.1
    );
    println!("\n  Microphones:");
    for s in sources.iter().filter(|s| !s.is_monitor) {
        println!("    {}  [{}]\n      {}", s.name, s.spec, s.description);
    }
    println!("\n  Output monitors (system audio loopback):");
    for s in sources.iter().filter(|s| s.is_monitor) {
        println!("    {}  [{}]\n      {}", s.name, s.spec, s.description);
    }

    println!("\ncpal (for comparison)");
    for host_id in cpal::available_hosts() {
        let host = cpal::host_from_id(host_id)?;
        println!("  host {:?}", host_id);
        for dev in host.devices()? {
            let dir = match (dev.supports_input(), dev.supports_output()) {
                (true, true) => "in+out",
                (true, false) => "in",
                (false, true) => "out",
                _ => "-",
            };
            println!("    [{dir:>6}] {}", device_label(&dev));
        }
    }
    Ok(())
}

fn device_label(dev: &cpal::Device) -> String {
    dev.description()
        .map(|d| d.to_string())
        .unwrap_or_else(|e| format!("<{e}>"))
}

/// Returns ((default_source, default_sink), sources) via the PulseAudio introspection API.
fn pulse_sources() -> Result<((String, String), Vec<SourceRow>)> {
    let mut mainloop = Mainloop::new().ok_or_else(|| anyhow!("pulse mainloop"))?;
    let mut ctx =
        Context::new(&mainloop, "veritas-spike").ok_or_else(|| anyhow!("pulse context"))?;
    ctx.connect(None, ContextFlags::NOFLAGS, None)?;
    loop {
        iterate(&mut mainloop)?;
        match ctx.get_state() {
            ContextState::Ready => break,
            ContextState::Failed | ContextState::Terminated => bail!("pulse context failed"),
            _ => {}
        }
    }

    let defaults = Rc::new(RefCell::new((String::new(), String::new())));
    let d = defaults.clone();
    let op = ctx.introspect().get_server_info(move |info| {
        *d.borrow_mut() = (
            info.default_source_name
                .as_deref()
                .unwrap_or_default()
                .to_string(),
            info.default_sink_name
                .as_deref()
                .unwrap_or_default()
                .to_string(),
        );
    });
    while op.get_state() == OpState::Running {
        iterate(&mut mainloop)?;
    }

    let rows = Rc::new(RefCell::new(Vec::new()));
    let r = rows.clone();
    let op = ctx.introspect().get_source_info_list(move |res| {
        if let ListResult::Item(src) = res {
            r.borrow_mut().push(SourceRow {
                name: src.name.as_deref().unwrap_or_default().to_string(),
                description: src.description.as_deref().unwrap_or_default().to_string(),
                is_monitor: src.monitor_of_sink.is_some(),
                spec: src.sample_spec.print(),
            });
        }
    });
    while op.get_state() == OpState::Running {
        iterate(&mut mainloop)?;
    }
    ctx.disconnect();

    let defaults = defaults.borrow().clone();
    let rows = rows.take();
    Ok((defaults, rows))
}

fn iterate(mainloop: &mut Mainloop) -> Result<()> {
    match mainloop.iterate(true) {
        IterateResult::Success(_) => Ok(()),
        IterateResult::Quit(_) => bail!("pulse mainloop quit"),
        IterateResult::Err(e) => bail!("pulse mainloop error: {e}"),
    }
}

// ---------------------------------------------------------------- record

struct TrackStats {
    label: &'static str,
    path: PathBuf,
    frames: u64,
    reads: u64,
    max_read_gap: Duration,
}

fn record(args: &[String]) -> Result<()> {
    let mut seconds = 10u64;
    let mut mic = "@DEFAULT_SOURCE@".to_string();
    let mut system = "@DEFAULT_MONITOR@".to_string();
    let mut out = PathBuf::from("recordings");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mic" => mic = it.next().context("--mic needs a value")?.clone(),
            "--system" => system = it.next().context("--system needs a value")?.clone(),
            "--out" => out = it.next().context("--out needs a value")?.into(),
            n => seconds = n.parse().with_context(|| format!("bad seconds: {n}"))?,
        }
    }
    std::fs::create_dir_all(&out)?;

    let stop = Arc::new(AtomicBool::new(false));
    let tracks = [
        ("mic", mic, out.join("mic.wav")),
        ("system", system, out.join("system.wav")),
    ];
    println!("recording {seconds}s at {RATE} Hz / {CHANNELS} ch f32 ...");

    // Open both streams before starting the clock so they begin together.
    let mut opened = Vec::new();
    for (label, source, path) in tracks {
        let simple =
            open_source(&source).with_context(|| format!("open {label} source {source}"))?;
        println!("  {label}: {source} -> {}", path.display());
        opened.push((label, simple, path));
    }

    let started = Instant::now();
    let handles: Vec<_> = opened
        .into_iter()
        .map(|(label, simple, path)| {
            let stop = stop.clone();
            thread::spawn(move || capture(label, simple, &path, &stop))
        })
        .collect();

    thread::sleep(Duration::from_secs(seconds));
    stop.store(true, Ordering::Relaxed);
    let wall = started.elapsed();

    println!("\nwall clock: {:.3}s", wall.as_secs_f64());
    let mut results = Vec::new();
    for h in handles {
        let s = h.join().map_err(|_| anyhow!("capture thread panicked"))??;
        let secs = s.frames as f64 / RATE as f64;
        println!(
            "  {:<6} {:>9} frames = {:.3}s  ({} reads, max gap between reads {:.1} ms)  {}",
            s.label,
            s.frames,
            secs,
            s.reads,
            s.max_read_gap.as_secs_f64() * 1000.0,
            s.path.display()
        );
        results.push(secs);
    }
    if let [a, b] = results[..] {
        println!("  drift between tracks: {:.1} ms", (a - b).abs() * 1000.0);
    }
    Ok(())
}

fn open_source(source: &str) -> Result<Simple> {
    let spec = Spec {
        format: Format::F32le,
        rate: RATE,
        channels: CHANNELS,
    };
    let bytes_per_chunk = (CHUNK_FRAMES * CHANNELS as usize * 4) as u32;
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: bytes_per_chunk,
    };
    Simple::new(
        None,
        "veritas-spike",
        Direction::Record,
        Some(source),
        "capture",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(|e| anyhow!("{e}"))
}

fn capture(
    label: &'static str,
    simple: Simple,
    path: &Path,
    stop: &AtomicBool,
) -> Result<TrackStats> {
    let spec = hound::WavSpec {
        channels: CHANNELS as u16,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec)?;
    let mut buf = vec![0u8; CHUNK_FRAMES * CHANNELS as usize * 4];
    let mut stats = TrackStats {
        label,
        path: path.to_owned(),
        frames: 0,
        reads: 0,
        max_read_gap: Duration::ZERO,
    };
    let mut last = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        simple
            .read(&mut buf)
            .map_err(|e| anyhow!("{label} read: {e}"))?;
        let now = Instant::now();
        if stats.reads > 0 {
            stats.max_read_gap = stats.max_read_gap.max(now - last);
        }
        last = now;
        for s in buf.as_chunks::<4>().0 {
            wav.write_sample(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))?;
        }
        stats.frames += CHUNK_FRAMES as u64;
        stats.reads += 1;
    }
    wav.finalize()?;
    Ok(stats)
}

// ---------------------------------------------------------------- record-cpal

fn record_cpal(args: &[String]) -> Result<()> {
    let mut seconds = 10u64;
    let mut out = PathBuf::from("recordings");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = it.next().context("--out needs a value")?.into(),
            n => seconds = n.parse().with_context(|| format!("bad seconds: {n}"))?,
        }
    }
    std::fs::create_dir_all(&out)?;

    let host = cpal::host_from_id(cpal::HostId::PipeWire)?;
    let mic = host
        .default_input_device()
        .context("no default input device")?;
    let system = host
        .default_output_device()
        .context("no default output device")?;
    println!(
        "recording {seconds}s with cpal {:?} host at {RATE} Hz / {CHANNELS} ch f32 ...",
        host.id()
    );

    let stop = Arc::new(AtomicBool::new(false));
    let mut streams = Vec::new();
    let mut writers = Vec::new();
    for (label, dev, path) in [
        ("mic", &mic, out.join("mic-cpal.wav")),
        ("system", &system, out.join("system-cpal.wav")),
    ] {
        println!("  {label}: {} -> {}", device_label(dev), path.display());
        // 2 s of headroom so a slow disk never makes the callback drop samples.
        let (mut tx, rx) = rtrb::RingBuffer::<f32>::new(RATE as usize * CHANNELS as usize * 2);
        let dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let dropped_cb = dropped.clone();
        let config = cpal::StreamConfig {
            channels: CHANNELS as u16,
            sample_rate: RATE,
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = dev.build_input_stream(
            config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                // Realtime thread: no locks, no allocation, never block.
                let n = data.len().min(tx.slots());
                if let Ok(chunk) = tx.write_chunk_uninit(n) {
                    chunk.fill_from_iter(data[..n].iter().copied());
                }
                if n < data.len() {
                    dropped_cb.fetch_add((data.len() - n) as u64, Ordering::Relaxed);
                }
            },
            move |e| eprintln!("{label} stream error: {e}"),
            Some(Duration::from_secs(5)),
        )?;
        streams.push(stream);
        let stop = stop.clone();
        writers.push(thread::spawn(move || {
            drain_to_wav(label, rx, &path, &stop, dropped)
        }));
    }

    for s in &streams {
        s.play()?;
    }
    let started = Instant::now();
    thread::sleep(Duration::from_secs(seconds));
    drop(streams);
    stop.store(true, Ordering::Relaxed);
    let wall = started.elapsed();

    println!("\nwall clock: {:.3}s", wall.as_secs_f64());
    let mut results = Vec::new();
    for w in writers {
        let (s, dropped) = w.join().map_err(|_| anyhow!("writer thread panicked"))??;
        let secs = s.frames as f64 / RATE as f64;
        println!(
            "  {:<6} {:>9} frames = {:.3}s  (dropped samples: {dropped})  {}",
            s.label,
            s.frames,
            secs,
            s.path.display()
        );
        results.push(secs);
    }
    if let [a, b] = results[..] {
        println!("  drift between tracks: {:.1} ms", (a - b).abs() * 1000.0);
    }
    Ok(())
}

fn drain_to_wav(
    label: &'static str,
    mut rx: rtrb::Consumer<f32>,
    path: &Path,
    stop: &AtomicBool,
    dropped: Arc<std::sync::atomic::AtomicU64>,
) -> Result<(TrackStats, u64)> {
    let spec = hound::WavSpec {
        channels: CHANNELS as u16,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec)?;
    let mut samples = 0u64;
    loop {
        let stopping = stop.load(Ordering::Relaxed);
        let n = rx.slots();
        if n > 0 {
            let chunk = rx.read_chunk(n)?;
            let (a, b) = chunk.as_slices();
            for &s in a.iter().chain(b) {
                wav.write_sample(s)?;
            }
            chunk.commit_all();
            samples += n as u64;
        } else if stopping {
            break;
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
    wav.finalize()?;
    let stats = TrackStats {
        label,
        path: path.to_owned(),
        frames: samples / CHANNELS as u64,
        reads: 0,
        max_read_gap: Duration::ZERO,
    };
    Ok((stats, dropped.load(Ordering::Relaxed)))
}
