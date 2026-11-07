//! Windows specifics (WASAPI loopback on output devices). Filled in by VRT-11.

use super::SourceKind;

pub fn is_virtual_device(_name: &str) -> bool {
    false
}

/// Not needed yet: WASAPI loopback asks no consent (VRT-11 may add the microphone setting).
pub fn privacy_settings_url(_kind: SourceKind) -> Option<&'static str> {
    None
}
