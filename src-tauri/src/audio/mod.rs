//! Audio capture (doc-2 §1.2): one track per source (mic and system loopback) via cpal,
//! realtime callbacks push into lock-free `rtrb` ring buffers, consumers record FLAC and
//! feed the transcription engine. Platform specifics live in the `cfg` modules below.

mod capture;
pub(crate) mod flac;
mod import;
pub(crate) mod levels;
mod mixdown;
mod monitor;
mod recorder;
mod recover;
pub(crate) mod resample;
mod ring;
mod sources;
mod waveform;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

pub use capture::list_sources;
pub use import::import_to_flac;
pub use mixdown::{MixSummary, decode_mono, mixdown_with_progress};
pub use monitor::Monitor;
pub use recorder::{Feed, Gap, Recording, Stopped, TrackSummary, track_file_names};
pub use recover::repair_flac;
pub use sources::{SourceInfo, SourceKind};
pub use waveform::{waveform, waveform_from_power};
