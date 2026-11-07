//! Audio source descriptions and the mapping from OS devices to Manent sources.

use serde::{Deserialize, Serialize};

/// What a source captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A microphone (input device). The mic track is "Me".
    Mic,
    /// System audio: an input stream on an output device (sink monitor / WASAPI loopback).
    Loopback,
}

/// A source the user can record, as shown in the UI (`src/lib/ipc.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceInfo {
    /// Stable cpal device id (`DeviceId` display form), used to open the source again.
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    /// The OS default device for this kind.
    pub is_default: bool,
}

/// Facts about one OS device, gathered from cpal.
#[derive(Debug, Clone)]
pub struct DeviceFacts {
    pub id: String,
    pub name: String,
    pub supports_input: bool,
    pub supports_output: bool,
    /// Placeholder devices that only alias the default (PipeWire: `default_input`, ...).
    pub is_virtual: bool,
}

/// Display names for the OS-default aliases (PipeWire), which follow the default device.
pub const SYSTEM_DEFAULT_MIC: &str = "System default microphone";
pub const SYSTEM_DEFAULT_OUTPUT: &str = "System default output";

/// Map OS devices to sources. Output devices become loopback sources (even when they also
/// report input, like PipeWire sinks); input-only devices become microphones. When the OS
/// default is a virtual alias (PipeWire `default_input`/`default_output`), that alias is
/// listed as "System default ..." so recording follows the OS default; other aliases are skipped.
pub fn classify(
    devices: &[DeviceFacts],
    default_input: Option<&str>,
    default_output: Option<&str>,
) -> Vec<SourceInfo> {
    let mut sources: Vec<SourceInfo> = devices
        .iter()
        .filter_map(|d| {
            if d.is_virtual {
                let (kind, name) = if default_input == Some(d.id.as_str()) {
                    (SourceKind::Mic, SYSTEM_DEFAULT_MIC)
                } else if default_output == Some(d.id.as_str()) {
                    (SourceKind::Loopback, SYSTEM_DEFAULT_OUTPUT)
                } else {
                    return None;
                };
                return Some(SourceInfo {
                    id: d.id.clone(),
                    name: name.into(),
                    kind,
                    is_default: true,
                });
            }
            let (kind, default) = if d.supports_output {
                (SourceKind::Loopback, default_output)
            } else if d.supports_input {
                (SourceKind::Mic, default_input)
            } else {
                return None;
            };
            Some(SourceInfo {
                id: d.id.clone(),
                name: d.name.clone(),
                kind,
                is_default: default == Some(d.id.as_str()),
            })
        })
        .collect();
    // Microphones first, defaults first within a kind, then by name.
    sources.sort_by(|a, b| {
        (a.kind == SourceKind::Loopback, !a.is_default, &a.name).cmp(&(
            b.kind == SourceKind::Loopback,
            !b.is_default,
            &b.name,
        ))
    });
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, input: bool, output: bool) -> DeviceFacts {
        DeviceFacts {
            id: id.into(),
            name: format!("{id} name"),
            supports_input: input,
            supports_output: output,
            is_virtual: false,
        }
    }

    #[test]
    fn inputs_are_mics_and_outputs_are_loopback() {
        let devices = [
            dev("mic1", true, false),
            dev("speaker", false, true),
            dev("sink", true, true), // PipeWire sink: in+out
        ];
        let s = classify(&devices, None, None);
        let kind = |id: &str| s.iter().find(|x| x.id == id).unwrap().kind;
        assert_eq!(kind("mic1"), SourceKind::Mic);
        assert_eq!(kind("speaker"), SourceKind::Loopback);
        assert_eq!(kind("sink"), SourceKind::Loopback);
    }

    #[test]
    fn virtual_default_aliases_become_system_default_sources() {
        let alias = |id: &str, input: bool, output: bool| DeviceFacts {
            is_virtual: true,
            ..dev(id, input, output)
        };
        let devices = [
            alias("default_input", true, false),
            alias("default_output", false, true),
            alias("default_sink", true, true),
            dev("mic", true, false),
            dev("speaker", true, true),
        ];
        let s = classify(&devices, Some("default_input"), Some("default_output"));
        let defaults: Vec<_> = s
            .iter()
            .filter(|x| x.is_default)
            .map(|x| (x.id.as_str(), x.name.as_str(), x.kind))
            .collect();
        assert_eq!(
            defaults,
            [
                ("default_input", SYSTEM_DEFAULT_MIC, SourceKind::Mic),
                (
                    "default_output",
                    SYSTEM_DEFAULT_OUTPUT,
                    SourceKind::Loopback
                )
            ]
        );
        assert!(
            !s.iter().any(|x| x.id == "default_sink"),
            "other aliases skipped"
        );
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn virtual_and_unusable_devices_are_skipped() {
        let mut virt = dev("default_input", true, false);
        virt.is_virtual = true;
        let devices = [virt, dev("dead", false, false), dev("mic", true, false)];
        let s = classify(&devices, None, None);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, "mic");
    }

    #[test]
    fn defaults_are_flagged_and_sorted_first() {
        let devices = [
            dev("mic-a", true, false),
            dev("mic-b", true, false),
            dev("out-a", false, true),
            dev("out-b", false, true),
        ];
        let s = classify(&devices, Some("mic-b"), Some("out-b"));
        let order: Vec<_> = s.iter().map(|x| (x.id.as_str(), x.is_default)).collect();
        assert_eq!(
            order,
            [
                ("mic-b", true),
                ("mic-a", false),
                ("out-b", true),
                ("out-a", false)
            ]
        );
    }

    #[test]
    fn serializes_kind_in_lowercase_for_the_ui() {
        let s = classify(&[dev("mic", true, false)], Some("mic"), None);
        let json = serde_json::to_value(&s[0]).unwrap();
        assert_eq!(json["kind"], "mic");
        assert_eq!(json["is_default"], true);
    }
}
