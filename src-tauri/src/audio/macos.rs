//! macOS specifics (CoreAudio loopback via an aggregate device in cpal). Filled in by VRT-12.

pub fn is_virtual_device(_name: &str) -> bool {
    false
}
