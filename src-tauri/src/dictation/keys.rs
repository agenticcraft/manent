//! Which keys type which character (VRT-59): auto-paste types the dictated text with the
//! desktop's own keyboard layout, which the EIS connection hands over as an xkb keymap.
//! Characters the layout can't type (é on a US layout) are pasted instead.

use std::collections::HashMap;

use xkbcommon::xkb;

/// evdev key codes are xkb key codes minus 8.
const EVDEV_OFFSET: u32 = 8;

/// The keys for one character: hold `mods`, tap `key` (evdev codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stroke {
    pub key: u32,
    pub mods: Vec<u32>,
}

/// Characters to keys for one layout (group) and lock state (Caps Lock).
#[derive(Debug, Default, Clone)]
pub struct Keys {
    strokes: HashMap<char, Stroke>,
}

/// A piece of text that is typed, or pasted when the layout can't type it.
#[derive(Debug, PartialEq, Eq)]
pub enum Run<'a> {
    Type(Vec<&'a Stroke>),
    Paste(&'a str),
}

/// Modifier keys, by the keysym they produce without modifiers.
const MODIFIER_KEYSYMS: [u32; 5] = [
    xkb::keysyms::KEY_Shift_L,
    xkb::keysyms::KEY_Shift_R,
    xkb::keysyms::KEY_ISO_Level3_Shift,
    xkb::keysyms::KEY_ISO_Level5_Shift,
    xkb::keysyms::KEY_Mode_switch,
];

impl Keys {
    /// Compile the keymap text (xkb v1) the compositor sent.
    pub fn from_keymap(text: &str, group: u32, locked: u32) -> Option<Keys> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            text.trim_end_matches('\0').to_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(Keys::new(&keymap, group, locked))
    }

    pub fn new(keymap: &xkb::Keymap, group: u32, locked: u32) -> Keys {
        // Which key sets each modifier bit.
        let mut mod_keys: HashMap<u32, u32> = HashMap::new();
        keymap.key_for_each(|keymap, code| {
            let syms = keymap.key_get_syms_by_level(code, 0, 0);
            if !syms.iter().any(|s| MODIFIER_KEYSYMS.contains(&s.raw())) {
                return;
            }
            let mut state = xkb::State::new(keymap);
            state.update_key(code, xkb::KeyDirection::Down);
            let mask = state.serialize_mods(xkb::STATE_MODS_DEPRESSED);
            // Keymaps also have keys no keyboard has (<LVL3>, <MDSW>); prefer real ones.
            let virtual_key = |c: u32| {
                matches!(
                    keymap.key_get_name(c.into()),
                    Some("LVL3" | "LVL5" | "MDSW")
                )
            };
            for bit in 0..32 {
                if mask & (1 << bit) != 0 {
                    let entry = mod_keys.entry(1 << bit).or_insert(code.raw());
                    if virtual_key(*entry) && !virtual_key(code.raw()) {
                        *entry = code.raw();
                    }
                }
            }
        });

        let mut strokes: HashMap<char, Stroke> = HashMap::new();
        keymap.key_for_each(|keymap, code| {
            let layouts = keymap.num_layouts_for_key(code);
            if layouts == 0 || code.raw() < EVDEV_OFFSET {
                return;
            }
            let layout = group.min(layouts - 1);
            for level in 0..keymap.num_levels_for_key(code, layout) {
                let mut masks = [0; 16];
                let n = keymap.key_get_mods_for_level(code, layout, level, &mut masks);
                for &mask in &masks[..n.min(masks.len())] {
                    // Every modifier in the mask needs a key that sets it.
                    let mut mods = Vec::new();
                    let mut ok = true;
                    for bit in 0..32 {
                        if mask & (1 << bit) != 0 {
                            match mod_keys.get(&(1 << bit)) {
                                Some(&k) => mods.push(k - EVDEV_OFFSET),
                                None => ok = false,
                            }
                        }
                    }
                    if !ok {
                        continue;
                    }
                    // What the key really gives with these modifiers and the locks (Caps Lock).
                    let mut state = xkb::State::new(keymap);
                    state.update_mask(mask, 0, locked, 0, 0, group);
                    let Some(c) = char::from_u32(state.key_get_utf32(code)) else {
                        continue;
                    };
                    if c.is_control() && c != '\r' && c != '\t' {
                        continue;
                    }
                    let stroke = Stroke {
                        key: code.raw() - EVDEV_OFFSET,
                        mods,
                    };
                    // Fewest modifiers first, then the lowest key (the main row before the keypad).
                    let better = match strokes.get(&c) {
                        None => true,
                        Some(old) => (stroke.mods.len(), stroke.key) < (old.mods.len(), old.key),
                    };
                    if better {
                        strokes.insert(c, stroke);
                    }
                }
            }
        });
        Keys { strokes }
    }

    pub fn stroke(&self, c: char) -> Option<&Stroke> {
        // Return types a new line.
        let c = if c == '\n' { '\r' } else { c };
        self.strokes.get(&c)
    }

    /// Split `text` into what can be typed and what has to be pasted.
    pub fn runs<'a>(&'a self, text: &'a str) -> Vec<Run<'a>> {
        let mut runs: Vec<Run<'a>> = Vec::new();
        let mut paste_from: Option<usize> = None;
        for (i, c) in text.char_indices() {
            match self.stroke(c) {
                Some(stroke) => {
                    if let Some(from) = paste_from.take() {
                        runs.push(Run::Paste(&text[from..i]));
                    }
                    match runs.last_mut() {
                        Some(Run::Type(strokes)) => strokes.push(stroke),
                        _ => runs.push(Run::Type(vec![stroke])),
                    }
                }
                None => {
                    paste_from.get_or_insert(i);
                }
            }
        }
        if let Some(from) = paste_from {
            runs.push(Run::Paste(&text[from..]));
        }
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: u32 = 30;
    const KEY_E: u32 = 18;
    const KEY_1: u32 = 2;
    const KEY_SPACE: u32 = 57;
    const KEY_LEFTSHIFT: u32 = 42;
    const KEY_RIGHTALT: u32 = 100;
    /// The Lock modifier (Caps Lock) is bit 1 in xkb's core modifiers.
    const LOCK: u32 = 1 << 1;

    fn keys(layout: &str, variant: &str, locked: u32) -> Keys {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "pc105",
            layout,
            variant,
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("xkeyboard-config has this layout");
        // Through text, as the compositor sends it.
        let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        Keys::from_keymap(&text, 0, locked).expect("the keymap compiles")
    }

    fn stroke(key: u32, mods: &[u32]) -> Stroke {
        Stroke {
            key,
            mods: mods.to_vec(),
        }
    }

    #[test]
    fn letters_digits_and_shifted_characters() {
        let k = keys("us", "", 0);
        assert_eq!(k.stroke('a'), Some(&stroke(KEY_A, &[])));
        assert_eq!(k.stroke('A'), Some(&stroke(KEY_A, &[KEY_LEFTSHIFT])));
        assert_eq!(
            k.stroke('1'),
            Some(&stroke(KEY_1, &[])),
            "main row, not the keypad"
        );
        assert_eq!(k.stroke('!'), Some(&stroke(KEY_1, &[KEY_LEFTSHIFT])));
        assert_eq!(k.stroke(' '), Some(&stroke(KEY_SPACE, &[])));
        assert_eq!(k.stroke('é'), None, "a US layout has no é");
    }

    #[test]
    fn caps_lock_flips_letters() {
        let k = keys("us", "", LOCK);
        assert_eq!(k.stroke('a'), Some(&stroke(KEY_A, &[KEY_LEFTSHIFT])));
        assert_eq!(k.stroke('A'), Some(&stroke(KEY_A, &[])));
        assert_eq!(k.stroke('1'), Some(&stroke(KEY_1, &[])));
    }

    #[test]
    fn altgr_characters() {
        let k = keys("us", "altgr-intl", 0);
        assert_eq!(k.stroke('é'), Some(&stroke(KEY_E, &[KEY_RIGHTALT])));
        assert_eq!(k.stroke('e'), Some(&stroke(KEY_E, &[])));
    }

    #[test]
    fn other_layouts_put_letters_elsewhere() {
        // AZERTY: a is where q is on QWERTY.
        let k = keys("fr", "", 0);
        assert_eq!(k.stroke('a'), Some(&stroke(16, &[])));
    }

    #[test]
    fn text_splits_into_typed_and_pasted_runs() {
        let k = keys("us", "", 0);
        let runs = k.runs("ideeën zijn ok");
        assert_eq!(runs.len(), 3);
        assert!(matches!(&runs[0], Run::Type(s) if s.len() == 4));
        assert_eq!(runs[1], Run::Paste("ë"));
        assert!(matches!(&runs[2], Run::Type(s) if s.len() == 9));
        assert!(matches!(
            k.runs("“hé”").as_slice(),
            [Run::Paste("“"), Run::Type(_), Run::Paste("é”")]
        ));
        assert!(k.runs("").is_empty());
    }
}
