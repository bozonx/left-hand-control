// Resolve a character literal (e.g. '@', '#', '{') to a concrete
// (evdev keycode + modifier list) pair, using libxkbcommon compiled with
// the user's currently active XKB layout.
//
// This is how we make "type a symbol" work reliably on arbitrary keyboard
// layouts: instead of hardcoding US positions, we ask xkbcommon where the
// keysym lives in the active keymap, and emit the corresponding keycode
// plus Shift / AltGr as needed.
//
// Limitations:
//   * We only look inside the currently active layout group. If a symbol
//     is not present there (e.g. '@' on a pure Russian layout), `resolve`
//     returns None and the caller should fall back / warn.
//   * We try four modifier combinations in order of preference:
//       (none, Shift, AltGr, Shift+AltGr).
//     That covers 99% of Latin/Cyrillic layouts; more exotic levels (like
//     Shift+AltGr+something) are intentionally not searched to keep the
//     emitted stroke predictable.

#![cfg(target_os = "linux")]

use std::collections::HashMap;

use evdev::Key;
use xkbcommon::xkb;

use super::action::Keystroke;
use super::keys::char_to_key;
use super::layout::LayoutInfo;

/// Resolve a single-character literal to a keystroke, preferring the
/// layout-aware `SymbolResolver` and falling back to the hardcoded
/// US-layout table in `keys::char_to_key` when the resolver either is
/// unavailable or does not know the character in the current layout.
pub fn resolve_literal(
    resolver: Option<&mut SymbolResolver>,
    ch: char,
) -> Option<Keystroke> {
    if let Some(r) = resolver {
        if let Some(ks) = r.resolve(ch) {
            return Some(ks);
        }
    }
    let (shift, key) = char_to_key(ch)?;
    let mods = if shift { vec![Key::KEY_LEFTSHIFT] } else { vec![] };
    Some(Keystroke { mods, key })
}

pub struct SymbolResolver {
    keymap: xkb::Keymap,
    shift_mask: xkb::ModMask,
    altgr_mask: xkb::ModMask,
    cache: HashMap<char, Option<Keystroke>>,
    info: LayoutInfo,
}

impl SymbolResolver {
    pub fn new(info: LayoutInfo) -> Result<Self, String> {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &ctx,
            "evdev",
            &info.model,
            &info.layout,
            &info.variant,
            info.options.clone(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| {
            format!(
                "xkb: failed to compile keymap (model={} layout={} variant={})",
                info.model, info.layout, info.variant
            )
        })?;

        let shift_idx = keymap.mod_get_index(xkb::MOD_NAME_SHIFT);
        let altgr_idx = keymap.mod_get_index(xkb::MOD_NAME_ISO_LEVEL3_SHIFT);
        let shift_mask: xkb::ModMask = if shift_idx != xkb::MOD_INVALID {
            1u32 << shift_idx
        } else {
            0
        };
        let altgr_mask: xkb::ModMask = if altgr_idx != xkb::MOD_INVALID {
            1u32 << altgr_idx
        } else {
            0
        };

        eprintln!(
            "[mapper] symbols: keymap ready layout={} variant={} model={} shift_mask=0x{:x} altgr_mask=0x{:x}",
            info.layout, info.variant, info.model, shift_mask, altgr_mask
        );

        Ok(Self {
            keymap,
            shift_mask,
            altgr_mask,
            cache: HashMap::new(),
            info,
        })
    }

    pub fn info(&self) -> &LayoutInfo {
        &self.info
    }

    /// Resolve a literal character to a concrete keystroke in the current
    /// layout. Returns None if the character's keysym is not reachable in
    /// this layout via (none / Shift / AltGr / Shift+AltGr).
    pub fn resolve(&mut self, ch: char) -> Option<Keystroke> {
        if let Some(v) = self.cache.get(&ch) {
            return v.clone();
        }
        let target = xkb::utf32_to_keysym(ch as u32);
        let res = self.lookup(target);
        if res.is_none() {
            eprintln!(
                "[mapper] symbols: {:?} not found in layout {}({})",
                ch, self.info.layout, self.info.variant
            );
        }
        self.cache.insert(ch, res.clone());
        res
    }

    fn lookup(&self, target: xkb::Keysym) -> Option<Keystroke> {
        // Keysym value 0 = NoSymbol → nothing to look for.
        if target.raw() == 0 {
            return None;
        }

        let combos: [(xkb::ModMask, Vec<Key>); 4] = [
            (0, vec![]),
            (self.shift_mask, vec![Key::KEY_LEFTSHIFT]),
            (self.altgr_mask, vec![Key::KEY_RIGHTALT]),
            (
                self.shift_mask | self.altgr_mask,
                vec![Key::KEY_LEFTSHIFT, Key::KEY_RIGHTALT],
            ),
        ];

        let min_kc: u32 = self.keymap.min_keycode().raw();
        let max_kc: u32 = self.keymap.max_keycode().raw();

        for (mask, mods) in &combos {
            // Skip combos that require modifiers missing from this layout.
            if !mods.is_empty() && *mask == 0 {
                continue;
            }
            let mut state = xkb::State::new(&self.keymap);
            state.update_mask(*mask, 0, 0, 0, 0, 0);
            for kc in min_kc..=max_kc {
                if kc < 8 {
                    // evdev code = xkb keycode - 8; below 8 is reserved.
                    continue;
                }
                let sym = state.key_get_one_sym(xkb::Keycode::new(kc));
                if sym == target {
                    let evdev_code = (kc - 8) as u16;
                    return Some(Keystroke {
                        mods: mods.clone(),
                        key: Key::new(evdev_code),
                    });
                }
            }
        }
        None
    }
}
