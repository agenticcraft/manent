//! macOS specifics (CoreAudio loopback via an aggregate device in cpal). Filled in by VRT-12.

use super::SourceKind;

pub fn is_virtual_device(_name: &str) -> bool {
    false
}

/// System Settings > Privacy & Security, at the list that grants this kind of capture.
pub fn privacy_settings_url(kind: SourceKind) -> Option<&'static str> {
    Some(match kind {
        SourceKind::Mic => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        SourceKind::Loopback => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AudioCapture"
        }
    })
}
