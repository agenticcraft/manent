//! The parts of libparakeet's C API (include/parakeet_capi.h, ABI v10) that Veritas uses.
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
    last_error: unsafe extern "C" fn(Ctx) -> *const c_char,
    free_string: unsafe extern "C" fn(*mut c_char),
    transcribe_vad_with:
        unsafe extern "C" fn(Ctx, Ctx, *const c_char, c_int, *const c_char) -> *mut c_char,
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
                "libparakeet ABI v{abi}, Veritas needs v{ABI}: download the engine again"
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
            last_error: sym!("parakeet_capi_last_error"),
            free_string: sym!("parakeet_capi_free_string"),
            transcribe_vad_with: sym!("parakeet_capi_transcribe_path_json_vad_with"),
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

    /// Words with timestamps for a WAV file. Audio over 30 s is cut at pauses by the
    /// Silero VAD component (the 0.6B model has no VAD head).
    pub fn transcribe(&self, wav: &Path) -> Result<Value, String> {
        let path = c_path(wav)?;
        let p = unsafe {
            (self.transcribe_vad_with)(self.asr.0, self.vad.0, path.as_ptr(), 0, std::ptr::null())
        };
        self.json(p, self.asr.0, "transcribe")
    }

    /// Speaker segments plus one embedding per usable speaker (speaker_profiles v1).
    pub fn diarize(&self, samples: &[f32], sample_rate: u32) -> Result<Value, String> {
        if samples.is_empty() {
            return Err("diarize: no audio".into());
        }
        let n = c_int::try_from(samples.len()).map_err(|_| "diarize: audio too long")?;
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
