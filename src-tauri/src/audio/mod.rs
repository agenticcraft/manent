//! Audio capture (doc-2 §1.2): one track per source (mic and system loopback) via cpal,
//! realtime callbacks push into lock-free `rtrb` ring buffers, consumers record FLAC and
//! feed the transcription engine. Platform specifics live in the `cfg` modules below.

mod capture;
mod flac;
mod levels;
mod mixdown;
mod monitor;
mod recorder;
mod resample;
mod ring;
mod sources;

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
pub use mixdown::mixdown;
pub use monitor::Monitor;
pub use recorder::{Recording, TrackSummary};
pub use sources::{SourceInfo, SourceKind};
