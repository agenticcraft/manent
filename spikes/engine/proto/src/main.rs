//! VRT-29 prototype: call libparakeet (parakeet.cpp C API) directly, without LocalAI.
//!
//! Usage: proto <libparakeet.so> <bundle.gguf> <audio.wav> <out prefix>
//! Writes <out>.transcript.json (transcribe_and_diarize_json) and <out>.profiles.json
//! (diarize_profiles_pcm_json) and prints load and processing times.

use std::ffi::{CStr, CString, c_char, c_float, c_int, c_void};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use libloading::{Library, Symbol};

type Ctx = *mut c_void;

struct Api<'l> {
    abi_version: Symbol<'l, unsafe extern "C" fn() -> c_int>,
    load_component: Symbol<'l, unsafe extern "C" fn(*const c_char, *const c_char) -> Ctx>,
    load_error: Symbol<'l, unsafe extern "C" fn() -> *const c_char>,
    last_error: Symbol<'l, unsafe extern "C" fn(Ctx) -> *const c_char>,
    free: Symbol<'l, unsafe extern "C" fn(Ctx)>,
    free_string: Symbol<'l, unsafe extern "C" fn(*mut c_char)>,
    transcribe_path_json_vad_with: Symbol<
        'l,
        unsafe extern "C" fn(Ctx, Ctx, *const c_char, c_int, *const c_char) -> *mut c_char,
    >,
    transcribe_pcm_batch_json: Symbol<
        'l,
        unsafe extern "C" fn(Ctx, *const c_float, *const c_int, c_int, c_int, c_int) -> *mut c_char,
    >,
    transcribe_and_diarize_json:
        Symbol<'l, unsafe extern "C" fn(Ctx, Ctx, *const c_float, c_int, c_int) -> *mut c_char>,
    diarize_profiles_pcm_json: Symbol<
        'l,
        unsafe extern "C" fn(
            Ctx,
            Ctx,
            *mut c_void,
            *const c_float,
            c_int,
            c_int,
            c_float,
            c_float,
        ) -> *mut c_char,
    >,
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [lib_path, bundle, wav, out] = args.as_slice() else {
        bail!("usage: proto <libparakeet.so> <bundle.gguf> <audio.wav> <out prefix>");
    };
    let t = Instant::now();
    let lib = unsafe { Library::new(lib_path) }.context("dlopen libparakeet")?;
    let api = unsafe {
        Api {
            abi_version: lib.get(b"parakeet_capi_abi_version")?,
            load_component: lib.get(b"parakeet_capi_load_component")?,
            load_error: lib.get(b"parakeet_capi_load_error")?,
            last_error: lib.get(b"parakeet_capi_last_error")?,
            free: lib.get(b"parakeet_capi_free")?,
            free_string: lib.get(b"parakeet_capi_free_string")?,
            transcribe_path_json_vad_with: lib
                .get(b"parakeet_capi_transcribe_path_json_vad_with")?,
            transcribe_pcm_batch_json: lib.get(b"parakeet_capi_transcribe_pcm_batch_json")?,
            transcribe_and_diarize_json: lib.get(b"parakeet_capi_transcribe_and_diarize_json")?,
            diarize_profiles_pcm_json: lib.get(b"parakeet_capi_diarize_profiles_pcm_json")?,
        }
    };
    let abi = unsafe { (api.abi_version)() };
    println!(
        "libparakeet ABI v{abi}, dlopen {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    if abi != 10 {
        bail!("expected ABI v10");
    }

    let bundle_c = CString::new(bundle.as_str())?;
    let load = |component: &str| -> Result<Ctx> {
        let c = CString::new(component)?;
        let ctx = unsafe { (api.load_component)(bundle_c.as_ptr(), c.as_ptr()) };
        if ctx.is_null() {
            bail!("load {component}: {}", cstr(unsafe { (api.load_error)() }));
        }
        Ok(ctx)
    };
    let t = Instant::now();
    let asr = load("asr")?;
    let diar = load("diar")?;
    let voice = load("voice")?;
    let vad = load("vad")?;
    println!(
        "loaded asr + diar + voice + vad components in {:.1} s",
        t.elapsed().as_secs_f64()
    );

    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
    };
    let ch = spec.channels as usize;
    let mono: Vec<f32> = samples
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    let seconds = mono.len() as f64 / spec.sample_rate as f64;
    println!("audio: {seconds:.1} s at {} Hz", spec.sample_rate);

    // Live captions (VRT-32 design): 6 s chunks of PCM, one call each.
    if std::env::var_os("PROTO_LIVE").is_some() {
        let chunk = 6 * spec.sample_rate as usize;
        let mut times = Vec::new();
        let mut words = 0usize;
        for piece in mono.chunks(chunk).take(20) {
            let n = [piece.len() as c_int];
            let t = Instant::now();
            let j = unsafe {
                (api.transcribe_pcm_batch_json)(
                    asr,
                    piece.as_ptr(),
                    n.as_ptr(),
                    1,
                    spec.sample_rate as c_int,
                    0,
                )
            };
            if j.is_null() {
                bail!("pcm_batch_json: {}", cstr(unsafe { (api.last_error)(asr) }));
            }
            times.push(t.elapsed().as_secs_f64());
            let doc: serde_json::Value = serde_json::from_str(&cstr(j))?;
            unsafe { (api.free_string)(j) };
            words += doc[0]["words"].as_array().map_or(0, |w| w.len());
        }
        times.sort_by(f64::total_cmp);
        println!(
            "live 6 s chunks: {} chunks, {words} words, median {:.2} s, max {:.2} s per chunk",
            times.len(),
            times[times.len() / 2],
            times[times.len() - 1]
        );
        return Ok(());
    }

    let t = Instant::now();
    let wav_c = CString::new(wav.as_str())?;
    let asr_json = unsafe {
        (api.transcribe_path_json_vad_with)(asr, vad, wav_c.as_ptr(), 0, std::ptr::null())
    };
    if asr_json.is_null() {
        bail!(
            "transcribe_path_json_vad: {}",
            cstr(unsafe { (api.last_error)(asr) })
        );
    }
    let took = t.elapsed().as_secs_f64();
    let text = cstr(asr_json);
    unsafe { (api.free_string)(asr_json) };
    std::fs::write(format!("{out}.asr.json"), &text)?;
    println!(
        "transcribe_path_json_vad: {took:.1} s (RTF {:.3})",
        took / seconds
    );

    // transcribe_and_diarize drops speech (VRT-29 notes); kept only for comparison.
    if std::env::var_os("PROTO_SAS").is_some() {
        let t = Instant::now();
        let json = unsafe {
            (api.transcribe_and_diarize_json)(
                asr,
                diar,
                mono.as_ptr(),
                mono.len() as c_int,
                spec.sample_rate as c_int,
            )
        };
        if json.is_null() {
            bail!(
                "transcribe_and_diarize: {} / {}",
                cstr(unsafe { (api.last_error)(asr) }),
                cstr(unsafe { (api.last_error)(diar) })
            );
        }
        let took = t.elapsed().as_secs_f64();
        let text = cstr(json);
        unsafe { (api.free_string)(json) };
        std::fs::write(format!("{out}.transcript.json"), &text)?;
        println!(
            "transcribe_and_diarize: {took:.1} s (RTF {:.3})",
            took / seconds
        );
    }

    let t = Instant::now();
    let prof = unsafe {
        (api.diarize_profiles_pcm_json)(
            diar,
            voice,
            std::ptr::null_mut(),
            mono.as_ptr(),
            mono.len() as c_int,
            spec.sample_rate as c_int,
            0.0,
            0.0,
        )
    };
    if prof.is_null() {
        bail!(
            "diarize_profiles: {} / {}",
            cstr(unsafe { (api.last_error)(diar) }),
            cstr(unsafe { (api.last_error)(voice) })
        );
    }
    let took = t.elapsed().as_secs_f64();
    let text = cstr(prof);
    unsafe { (api.free_string)(prof) };
    std::fs::write(format!("{out}.profiles.json"), &text)?;
    println!("diarize_profiles: {took:.1} s (RTF {:.3})", took / seconds);

    unsafe {
        (api.free)(asr);
        (api.free)(diar);
        (api.free)(voice);
        (api.free)(vad);
    }
    Ok(())
}
