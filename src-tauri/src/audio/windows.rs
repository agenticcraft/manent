//! Windows specifics (WASAPI loopback on output devices). Filled in by VRT-11.

pub fn is_virtual_device(_name: &str) -> bool {
    false
}
