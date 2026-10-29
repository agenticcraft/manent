//! Linux (PipeWire) specifics. cpal's default host is the PipeWire host when the `pipewire`
//! feature is enabled; sinks show up as in+out devices and an input stream on a sink captures
//! its monitor (validated in VRT-1).

/// PipeWire exposes alias devices that follow the current default; we list real devices only.
pub fn is_virtual_device(name: &str) -> bool {
    matches!(
        name,
        "default_input" | "default_output" | "default_sink" | "default_source"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipewire_aliases_are_virtual() {
        assert!(is_virtual_device("default_sink"));
        assert!(is_virtual_device("default_input"));
        assert!(!is_virtual_device("Ryzen HD Audio Controller Speaker"));
    }
}
