//! PC scan codes (set 1, as reported by Windows) ↔ Linux key codes.
//!
//! Linux key codes 1..=83 and 86..=88 equal their set-1 scan codes, which is
//! where the `input-event-codes.h` numbering comes from. The remaining keys
//! need a table: `E0`-prefixed (extended) keys, the F13..F24 block and the
//! Japanese/Korean keys. Media and browser keys, Pause and Num Lock are
//! better identified by their virtual-key code, which Windows reports even
//! when a HID device provides no scan code.

use super::key_code::KeyCode;

/// Where a key comes from or goes to on Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeKey {
    /// Injected as a scan code so the target's keyboard layout applies.
    Scan { code: u16, extended: bool },
    /// Injected as a virtual key (with its scan code when it has one).
    Virtual { vk: u16, scan: u16, extended: bool },
}

const EXTENDED: &[(u16, KeyCode)] = &[
    (0x1c, KeyCode::KEY_KPENTER),
    (0x1d, KeyCode::KEY_RIGHTCTRL),
    (0x35, KeyCode::KEY_KPSLASH),
    (0x37, KeyCode::KEY_SYSRQ),
    (0x38, KeyCode::KEY_RIGHTALT),
    (0x47, KeyCode::KEY_HOME),
    (0x48, KeyCode::KEY_UP),
    (0x49, KeyCode::KEY_PAGEUP),
    (0x4b, KeyCode::KEY_LEFT),
    (0x4d, KeyCode::KEY_RIGHT),
    (0x4f, KeyCode::KEY_END),
    (0x50, KeyCode::KEY_DOWN),
    (0x51, KeyCode::KEY_PAGEDOWN),
    (0x52, KeyCode::KEY_INSERT),
    (0x53, KeyCode::KEY_DELETE),
    (0x5b, KeyCode::KEY_LEFTMETA),
    (0x5c, KeyCode::KEY_RIGHTMETA),
    (0x5d, KeyCode::KEY_COMPOSE),
    (0x5e, KeyCode::KEY_POWER),
    (0x5f, KeyCode::KEY_SLEEP),
    (0x63, KeyCode::KEY_WAKEUP),
];

const PLAIN: &[(u16, KeyCode)] = &[
    (0x59, KeyCode::KEY_KPEQUAL),
    (0x64, KeyCode::KEY_F13),
    (0x65, KeyCode::KEY_F14),
    (0x66, KeyCode::KEY_F15),
    (0x67, KeyCode::KEY_F16),
    (0x68, KeyCode::KEY_F17),
    (0x69, KeyCode::KEY_F18),
    (0x6a, KeyCode::KEY_F19),
    (0x6b, KeyCode::KEY_F20),
    (0x6c, KeyCode::KEY_F21),
    (0x6d, KeyCode::KEY_F22),
    (0x6e, KeyCode::KEY_F23),
    (0x70, KeyCode::KEY_KATAKANAHIRAGANA),
    (0x73, KeyCode::KEY_RO),
    (0x76, KeyCode::KEY_F24),
    (0x79, KeyCode::KEY_HENKAN),
    (0x7b, KeyCode::KEY_MUHENKAN),
    (0x7d, KeyCode::KEY_YEN),
    (0x7e, KeyCode::KEY_KPCOMMA),
    (0xf1, KeyCode::KEY_HANJA),
    (0xf2, KeyCode::KEY_HANGEUL),
];

/// Keys identified by virtual-key code: (vk, extended scan code, key).
const VIRTUAL: &[(u16, u16, KeyCode)] = &[
    (0x13, 0x45, KeyCode::KEY_PAUSE),
    (0x90, 0x45, KeyCode::KEY_NUMLOCK),
    (0xa6, 0x6a, KeyCode::KEY_BACK),
    (0xa7, 0x69, KeyCode::KEY_FORWARD),
    (0xa8, 0x67, KeyCode::KEY_REFRESH),
    (0xa9, 0x68, KeyCode::KEY_STOP),
    (0xaa, 0x65, KeyCode::KEY_SEARCH),
    (0xab, 0x66, KeyCode::KEY_BOOKMARKS),
    (0xac, 0x32, KeyCode::KEY_HOMEPAGE),
    (0xad, 0x20, KeyCode::KEY_MUTE),
    (0xae, 0x2e, KeyCode::KEY_VOLUMEDOWN),
    (0xaf, 0x30, KeyCode::KEY_VOLUMEUP),
    (0xb0, 0x19, KeyCode::KEY_NEXTSONG),
    (0xb1, 0x10, KeyCode::KEY_PREVIOUSSONG),
    (0xb2, 0x24, KeyCode::KEY_STOPCD),
    (0xb3, 0x22, KeyCode::KEY_PLAYPAUSE),
    (0xb4, 0x6c, KeyCode::KEY_MAIL),
    (0xb5, 0x6d, KeyCode::KEY_MEDIA),
    (0xb6, 0x6b, KeyCode::KEY_COMPUTER),
    (0xb7, 0x21, KeyCode::KEY_CALC),
];

/// Pause's make code is `E1 1D 45`; Num Lock shares `45` (non-extended
/// in the hardware sequence, reported extended by Windows).
fn virtual_scan_is_extended(vk: u16) -> bool {
    vk != 0x13 && vk != 0x90
}

/// Key for a keyboard event as a low-level hook reports it.
pub fn from_native(vk: u32, scan: u32, extended: bool) -> Option<KeyCode> {
    if let Some((_, _, key)) = VIRTUAL.iter().find(|(v, ..)| u32::from(*v) == vk) {
        return Some(*key);
    }
    let scan = u16::try_from(scan & 0xff).ok()?;
    if extended {
        return EXTENDED
            .iter()
            .find(|(code, _)| *code == scan)
            .map(|(_, key)| *key);
    }
    match scan {
        0x01..=0x53 | 0x56..=0x58 => Some(KeyCode::new(scan)),
        _ => PLAIN
            .iter()
            .find(|(code, _)| *code == scan)
            .map(|(_, key)| *key),
    }
}

/// How to inject `key`, or `None` for keys a PC keyboard cannot produce.
pub fn to_native(key: KeyCode) -> Option<NativeKey> {
    if let Some((vk, scan, _)) = VIRTUAL.iter().find(|(.., k)| *k == key) {
        return Some(NativeKey::Virtual {
            vk: *vk,
            scan: *scan,
            extended: virtual_scan_is_extended(*vk),
        });
    }
    if let Some((code, _)) = EXTENDED.iter().find(|(_, k)| *k == key) {
        return Some(NativeKey::Scan {
            code: *code,
            extended: true,
        });
    }
    if let Some((code, _)) = PLAIN.iter().find(|(_, k)| *k == key) {
        return Some(NativeKey::Scan {
            code: *code,
            extended: false,
        });
    }
    match key.code() {
        code @ (0x01..=0x53 | 0x56..=0x58) => Some(NativeKey::Scan {
            code,
            extended: false,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_block_uses_scan_codes_as_key_codes() {
        assert_eq!(from_native(0x41, 0x1e, false), Some(KeyCode::KEY_A));
        assert_eq!(from_native(0x1b, 0x01, false), Some(KeyCode::KEY_ESC));
        assert_eq!(from_native(0xa0, 0x2a, false), Some(KeyCode::KEY_LEFTSHIFT));
        assert_eq!(
            from_native(0xa1, 0x36, false),
            Some(KeyCode::KEY_RIGHTSHIFT)
        );
        assert_eq!(from_native(0x7a, 0x57, false), Some(KeyCode::KEY_F11));
        assert_eq!(from_native(0xe2, 0x56, false), Some(KeyCode::KEY_102ND));
        assert_eq!(from_native(0x7c, 0x64, false), Some(KeyCode::KEY_F13));
    }

    #[test]
    fn extended_keys_differ_from_their_keypad_twins() {
        assert_eq!(from_native(0x24, 0x47, true), Some(KeyCode::KEY_HOME));
        assert_eq!(from_native(0x67, 0x47, false), Some(KeyCode::KEY_KP7));
        assert_eq!(from_native(0x0d, 0x1c, true), Some(KeyCode::KEY_KPENTER));
        assert_eq!(from_native(0xa3, 0x1d, true), Some(KeyCode::KEY_RIGHTCTRL));
        assert_eq!(from_native(0xa5, 0x38, true), Some(KeyCode::KEY_RIGHTALT));
        assert_eq!(from_native(0x5b, 0x5b, true), Some(KeyCode::KEY_LEFTMETA));
        assert_eq!(from_native(0, 0x7f, true), None);
    }

    #[test]
    fn pause_num_lock_and_media_use_virtual_keys() {
        assert_eq!(from_native(0x13, 0x45, false), Some(KeyCode::KEY_PAUSE));
        assert_eq!(from_native(0x90, 0x45, true), Some(KeyCode::KEY_NUMLOCK));
        assert_eq!(from_native(0xaf, 0, false), Some(KeyCode::KEY_VOLUMEUP));
        assert_eq!(
            to_native(KeyCode::KEY_PAUSE),
            Some(NativeKey::Virtual {
                vk: 0x13,
                scan: 0x45,
                extended: false
            })
        );
    }

    #[test]
    fn every_native_key_round_trips() {
        for code in 0..=0x2ff {
            let key = KeyCode::new(code);
            let Some(native) = to_native(key) else {
                continue;
            };
            let back = match native {
                NativeKey::Scan { code, extended } => from_native(0, code.into(), extended),
                NativeKey::Virtual { vk, scan, extended } => {
                    from_native(vk.into(), scan.into(), extended)
                }
            };
            assert_eq!(back, Some(key), "{code:#x}");
        }
        assert!(to_native(KeyCode::KEY_A).is_some());
        assert!(to_native(KeyCode::BTN_LEFT).is_none());
    }
}
