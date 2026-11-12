//! The parts of libparakeet's C API (include/parakeet_capi.h, ABI v10) that Manent uses.
//! VRT-29 measured these calls; `transcribe_and_diarize*` is deliberately not used.

use std::ffi::{CStr, CString, c_char, c_float, c_int, c_void};
use std::path::Path;

use libloading::Library;
use serde_json::Value;

pub const ABI: c_int = 10;
/// Bundle components: ASR (empty name), Silero VAD, diarization, speaker encoder.
pub const COMPONENTS: [&str; 4] = ["asr", "vad", "diar", "voice"];

type Ctx = *mut c_void;

/// libparakeet contexts may be used from several threads at once (the header documents
/// calls on one context as thread safe; ASR calls run in parallel after set_concurrency).
struct Shared(Ctx);
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

pub struct Engine {
    pub abi: c_int,
    pub backends: c_int,
    asr: Shared,
    vad: Shared,
    diar: Shared,
    voice: Shared,
    /// The diarization and speaker (voice) contexts may not be used by two threads at once
    /// (parakeet_capi.h); requests run in parallel, so diarizations take turns (VRT-57).
    speakers: std::sync::Mutex<()>,
    last_error: unsafe extern "C" fn(Ctx) -> *const c_char,
    free_string: unsafe extern "C" fn(*mut c_char),
    transcribe_vad_with:
        unsafe extern "C" fn(Ctx, Ctx, *const c_char, c_int, *const c_char) -> *mut c_char,
    vad_path_json: unsafe extern "C" fn(Ctx, *const c_char, *const c_char) -> *mut c_char,
    transcribe_pcm_batch_json:
        unsafe extern "C" fn(Ctx, *const c_float, *const c_int, c_int, c_int, c_int) -> *mut c_char,
    diarize_profiles: unsafe extern "C" fn(
        Ctx,
        Ctx,
        *mut c_void,
        *const c_float,
        c_int,
        c_int,
        c_float,
        c_float,
    ) -> *mut c_char,
    // Keeps the function pointers above valid; never unloaded while the process runs.
    _lib: Library,
}

fn text(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn c_path(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

impl Engine {
    pub fn load(lib_path: &Path, model: &Path) -> Result<Self, String> {
        if !lib_path.is_file() {
            return Err(format!(
                "{} is missing: download the engine again",
                lib_path.display()
            ));
        }
        let lib = unsafe { Library::new(lib_path) }
            .map_err(|e| format!("loading {}: {e}", lib_path.display()))?;
        macro_rules! sym {
            ($name:literal) => {
                *unsafe { lib.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|e| format!("{}: {e}", $name))?
            };
        }
        let abi_version: unsafe extern "C" fn() -> c_int = sym!("parakeet_capi_abi_version");
        let abi = unsafe { abi_version() };
        if abi != ABI {
            return Err(format!(
                "libparakeet ABI v{abi}, Manent needs v{ABI}: download the engine again"
            ));
        }
        let load_component: unsafe extern "C" fn(*const c_char, *const c_char) -> Ctx =
            sym!("parakeet_capi_load_component");
        let load_error: unsafe extern "C" fn() -> *const c_char = sym!("parakeet_capi_load_error");
        let set_concurrency: unsafe extern "C" fn(Ctx, c_int, c_int) -> c_int =
            sym!("parakeet_capi_set_concurrency");

        let model_c = c_path(model)?;
        let load = |component: &str| -> Result<Shared, String> {
            // The ASR model is the bundle's default component (empty name).
            let name = CString::new(if component == "asr" { "" } else { component })
                .map_err(|e| e.to_string())?;
            let ctx = unsafe { load_component(model_c.as_ptr(), name.as_ptr()) };
            if ctx.is_null() {
                Err(format!(
                    "loading {component} from {}: {}",
                    model.display(),
                    text(unsafe { load_error() })
                ))
            } else {
                Ok(Shared(ctx))
            }
        };
        let asr = load("asr")?;
        let vad = load("vad")?;
        let diar = load("diar")?;
        let voice = load("voice")?;
        // Two ASR backends, so a live chunk doesn't queue behind a rolling pass. Each gets
        // half the cores (logical cores / 2 is about the physical count with SMT).
        let logical = std::thread::available_parallelism().map_or(4, |n| n.get()) as c_int;
        let backends = unsafe { set_concurrency(asr.0, 2, (logical / 4).max(1)) };

        Ok(Self {
            abi,
            backends,
            asr,
            vad,
            diar,
            voice,
            speakers: std::sync::Mutex::new(()),
            last_error: sym!("parakeet_capi_last_error"),
            free_string: sym!("parakeet_capi_free_string"),
            transcribe_vad_with: sym!("parakeet_capi_transcribe_path_json_vad_with"),
            vad_path_json: sym!("parakeet_capi_vad_path_json"),
            transcribe_pcm_batch_json: sym!("parakeet_capi_transcribe_pcm_batch_json"),
            diarize_profiles: sym!("parakeet_capi_diarize_profiles_pcm_json"),
            _lib: lib,
        })
    }

    /// Take ownership of a returned JSON string and parse it.
    fn json(&self, p: *mut c_char, ctx: Ctx, what: &str) -> Result<Value, String> {
        if p.is_null() {
            return Err(format!(
                "{what}: {}",
                text(unsafe { (self.last_error)(ctx) })
            ));
        }
        let s = text(p);
        unsafe { (self.free_string)(p) };
        serde_json::from_str(&s).map_err(|e| format!("{what}: bad JSON from libparakeet: {e}"))
    }

    /// Words with timestamps for a WAV file (`samples` are its mono samples). Audio over 30 s
    /// is cut at pauses by the Silero VAD component (the 0.6B model has no VAD head).
    ///
    /// Repair (VRT-17): within a piece that switches language, the model sometimes drops a
    /// whole sentence (seen with Dutch-English-Dutch). Speech regions of at least 1 s that got
    /// no words are transcribed again on their own and their words added.
    pub fn transcribe(&self, wav: &Path, samples: &[f32], rate: u32) -> Result<Value, String> {
        let path = c_path(wav)?;
        let p = unsafe {
            (self.transcribe_vad_with)(self.asr.0, self.vad.0, path.as_ptr(), 0, std::ptr::null())
        };
        let mut doc = self.json(p, self.asr.0, "transcribe")?;
        let p = unsafe { (self.vad_path_json)(self.vad.0, path.as_ptr(), std::ptr::null()) };
        let regions = self.json(p, self.vad.0, "vad")?;
        let words = doc["words"].as_array().cloned().unwrap_or_default();
        let span = |v: &Value| {
            (
                v["start"].as_f64().unwrap_or(0.0),
                v["end"].as_f64().unwrap_or(0.0),
            )
        };
        let missed = uncovered(
            regions["segments"]
                .as_array()
                .map(|a| a.iter().map(span).collect())
                .unwrap_or_default(),
            &words.iter().map(span).collect::<Vec<_>>(),
        );
        if missed.is_empty() {
            return Ok(doc);
        }
        let mut all = words;
        for (start, end) in &missed {
            let a = ((start - 0.3).max(0.0) * rate as f64) as usize;
            let b = (((end + 0.3) * rate as f64) as usize).min(samples.len());
            if b <= a {
                continue;
            }
            let piece = &samples[a..b];
            let n = [piece.len() as c_int];
            let p = unsafe {
                (self.transcribe_pcm_batch_json)(
                    self.asr.0,
                    piece.as_ptr(),
                    n.as_ptr(),
                    1,
                    rate as c_int,
                    0,
                )
            };
            let found = self.json(p, self.asr.0, "transcribe (repair)")?;
            let offset = a as f64 / rate as f64;
            for w in found[0]["words"].as_array().cloned().unwrap_or_default() {
                let mut w = w;
                for key in ["start", "end"] {
                    w[key] = serde_json::json!(w[key].as_f64().unwrap_or(0.0) + offset);
                }
                all.push(w);
            }
        }
        all.sort_by(|x, y| span(x).0.total_cmp(&span(y).0));
        doc["text"] = Value::String(
            all.iter()
                .filter_map(|w| w["w"].as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        doc["words"] = Value::Array(all);
        doc["repaired"] = serde_json::json!(missed.len());
        Ok(doc)
    }

    /// Speaker segments plus one embedding per usable speaker (speaker_profiles v1).
    pub fn diarize(&self, samples: &[f32], sample_rate: u32) -> Result<Value, String> {
        if samples.is_empty() {
            return Err("diarize: no audio".into());
        }
        let n = c_int::try_from(samples.len()).map_err(|_| "diarize: audio too long")?;
        let _turn = self.speakers.lock().unwrap_or_else(|e| e.into_inner());
        let p = unsafe {
            (self.diarize_profiles)(
                self.diar.0,
                self.voice.0,
                std::ptr::null_mut(),
                samples.as_ptr(),
                n,
                sample_rate as c_int,
                0.0,
                0.0,
            )
        };
        self.json(p, self.diar.0, "diarize")
    }
}

/// Speech regions (seconds) of at least 1 s that no word overlaps; regions less than 0.5 s
/// apart are joined first (a pause inside a sentence splits it into two regions).
pub fn uncovered(regions: Vec<(f64, f64)>, words: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut joined: Vec<(f64, f64)> = Vec::new();
    for (s, e) in regions {
        match joined.last_mut() {
            Some(last) if s - last.1 < 0.5 => last.1 = last.1.max(e),
            _ => joined.push((s, e)),
        }
    }
    joined
        .into_iter()
        .filter(|(s, e)| e - s >= 1.0)
        .filter(|(s, e)| !words.iter().any(|(ws, we)| ws < e && we > s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_speech_without_words() {
        let regions = vec![(0.0, 2.0), (3.0, 4.0), (4.2, 5.5), (7.0, 7.5), (9.0, 11.0)];
        let words = [(0.5, 0.9), (9.5, 10.0)];
        // 3-4 and 4.2-5.5 join into one 2.5 s region; 7-7.5 is too short.
        assert_eq!(uncovered(regions, &words), vec![(3.0, 5.5)]);
        assert!(uncovered(vec![(0.0, 3.0)], &[(2.9, 3.2)]).is_empty());
    }
}
