//! XKB keymaps: which key and level type each character.

use super::*;

pub(super) enum XkbContext {}
pub(super) enum XkbKeymap {}
pub(super) enum XkbState {}

#[repr(C)]
pub(super) struct XkbRuleNames {
    pub(super) rules: *const c_char,
    pub(super) model: *const c_char,
    pub(super) layout: *const c_char,
    pub(super) variant: *const c_char,
    pub(super) options: *const c_char,
}

pub(super) const XKB_KEYMAP_FORMAT_TEXT_V1: u32 = 1;

#[link(name = "xkbcommon")]
unsafe extern "C" {
    pub(super) fn xkb_utf32_to_keysym(ucs: u32) -> u32;
    pub(super) fn xkb_context_new(flags: u32) -> *mut XkbContext;
    pub(super) fn xkb_context_unref(ctx: *mut XkbContext);
    pub(super) fn xkb_keymap_new_from_names(
        ctx: *mut XkbContext,
        names: *const XkbRuleNames,
        flags: u32,
    ) -> *mut XkbKeymap;
    pub(super) fn xkb_keymap_new_from_buffer(
        ctx: *mut XkbContext,
        buffer: *const c_char,
        length: usize,
        format: u32,
        flags: u32,
    ) -> *mut XkbKeymap;
    pub(super) fn xkb_keymap_unref(keymap: *mut XkbKeymap);
    pub(super) fn xkb_keymap_min_keycode(keymap: *mut XkbKeymap) -> u32;
    pub(super) fn xkb_keymap_max_keycode(keymap: *mut XkbKeymap) -> u32;
    pub(super) fn xkb_state_new(keymap: *mut XkbKeymap) -> *mut XkbState;
    pub(super) fn xkb_state_unref(state: *mut XkbState);
    pub(super) fn xkb_state_update_mask(
        state: *mut XkbState,
        depressed_mods: u32,
        latched_mods: u32,
        locked_mods: u32,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) -> u32;
    pub(super) fn xkb_state_update_key(state: *mut XkbState, key: u32, direction: u32) -> u32;
    pub(super) fn xkb_state_key_get_one_sym(state: *mut XkbState, key: u32) -> u32;
    pub(super) fn xkb_state_serialize_mods(state: *mut XkbState, components: u32) -> u32;
    pub(super) fn xkb_state_serialize_layout(state: *mut XkbState, components: u32) -> u32;
}

// --- XKB keymap lookup for layout-aware text injection ------------------

// evdev keycodes for modifier keys used in text injection.
pub(super) const KEY_LEFTCTRL_EVDEV: u32 = evdev::KeyCode::KEY_LEFTCTRL.code() as u32;
pub(super) const KEY_LEFTSHIFT_EVDEV: u32 = evdev::KeyCode::KEY_LEFTSHIFT.code() as u32;
pub(super) const KEY_V_EVDEV: u32 = evdev::KeyCode::KEY_V.code() as u32;
pub(super) const KEY_INSERT_EVDEV: u32 = evdev::KeyCode::KEY_INSERT.code() as u32;

/// Modifiers and key of a paste shortcut as evdev codes.
pub(super) fn paste_keys(shortcut: PasteShortcut) -> (&'static [u32], u32) {
    match shortcut {
        PasteShortcut::ShiftInsert => (&[KEY_LEFTSHIFT_EVDEV], KEY_INSERT_EVDEV),
        PasteShortcut::CtrlV => (&[KEY_LEFTCTRL_EVDEV], KEY_V_EVDEV),
        PasteShortcut::CtrlShiftV => (&[KEY_LEFTCTRL_EVDEV, KEY_LEFTSHIFT_EVDEV], KEY_V_EVDEV),
    }
}

#[derive(Clone)]
pub(super) struct KeycodeEntry {
    pub(super) evdev: u32,
    pub(super) mods: Vec<u32>,
}

/// keysym → the key and level that type it.
pub(super) type KeyTable = HashMap<u32, KeycodeEntry>;

pub(super) type KeymapCache = Option<(String, Arc<KeyTable>)>;
pub(super) static KEYMAP_CACHE: Mutex<KeymapCache> = Mutex::new(None);

/// Key table of the active layout, rebuilt from its XKB name and variant.
/// Fails when the layout is unknown (only the KDE backend reports it) or
/// XKB cannot compile it: guessing a layout would type wrong characters.
pub(super) fn layout_key_table() -> Result<Arc<KeyTable>, TextInputFailure> {
    let layout = crate::runtime_state::keyboard_layout().ok_or(TextInputFailure::LayoutUnknown)?;
    let variant = layout.variant.trim();
    let caps_lock = caps_lock_state()?;
    let options = crate::keyboard_layout::xkb_options();
    let cache_key = format!("{}\0{variant}\0{caps_lock}\0{options}", layout.code);
    let mut cache = KEYMAP_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((key, table)) = cache.as_ref()
        && *key == cache_key
    {
        return Ok(Arc::clone(table));
    }
    let table = Arc::new(
        table_from_names_with_options(
            &layout.code,
            (!variant.is_empty()).then_some(variant),
            caps_lock,
            &options,
        )
        .ok_or(TextInputFailure::LayoutUnknown)?,
    );
    *cache = Some((cache_key, Arc::clone(&table)));
    Ok(table)
}

#[cfg(test)]
pub(super) fn table_from_names(layout: &str, variant: Option<&str>) -> Option<KeyTable> {
    table_from_names_with_caps(layout, variant, false)
}

#[cfg(test)]
pub(super) fn table_from_names_with_caps(
    layout: &str,
    variant: Option<&str>,
    caps: bool,
) -> Option<KeyTable> {
    table_from_names_with_options(layout, variant, caps, "")
}

pub(super) fn table_from_names_with_options(
    layout: &str,
    variant: Option<&str>,
    caps: bool,
    options: &str,
) -> Option<KeyTable> {
    let options = CString::new(options).ok()?;
    let layout_c = CString::new(layout).ok()?;
    let variant_c = match variant {
        Some(variant) => Some(CString::new(variant).ok()?),
        None => None,
    };
    let names = XkbRuleNames {
        rules: std::ptr::null(),
        model: std::ptr::null(),
        layout: layout_c.as_ptr(),
        variant: variant_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        options: options.as_ptr(),
    };
    // SAFETY: the context and keymap are checked for null and released
    // before returning; `names` points to strings that outlive the call.
    unsafe {
        let ctx = xkb_context_new(0);
        if ctx.is_null() {
            return None;
        }
        let keymap = xkb_keymap_new_from_names(ctx, &names, 0);
        xkb_context_unref(ctx);
        if keymap.is_null() {
            log::warn!("[text] XKB cannot compile layout {layout:?} variant {variant:?}");
            return None;
        }
        let mut locked = 0;
        if caps {
            let state = xkb_state_new(keymap);
            if state.is_null() {
                xkb_keymap_unref(keymap);
                return None;
            }
            // Caps Lock may be configured as Shift Lock; derive its actual action.
            let lock_key = (xkb_keymap_min_keycode(keymap)..=xkb_keymap_max_keycode(keymap))
                .find(|key| matches!(xkb_state_key_get_one_sym(state, *key), 0xffe5 | 0xffe6));
            if let Some(key) = lock_key {
                xkb_state_update_key(state, key, 1);
                xkb_state_update_key(state, key, 0);
                locked = xkb_state_serialize_mods(state, 1 << 2);
            }
            xkb_state_unref(state);
            if locked == 0 {
                xkb_keymap_unref(keymap);
                return None;
            }
        }
        let table = table_from_keymap(keymap, 0, locked);
        xkb_keymap_unref(keymap);
        Some(table)
    }
}

/// Key table of `group` in a keymap the compositor sent as XKB text.
pub(super) fn table_from_keymap_text(text: &[u8], group: u32, locked: u32) -> Option<KeyTable> {
    let text = text.split(|byte| *byte == 0).next().unwrap_or_default();
    // SAFETY: `text` outlives the call; context and keymap are checked for
    // null and released before returning.
    unsafe {
        let ctx = xkb_context_new(0);
        if ctx.is_null() {
            return None;
        }
        let keymap = xkb_keymap_new_from_buffer(
            ctx,
            text.as_ptr().cast(),
            text.len(),
            XKB_KEYMAP_FORMAT_TEXT_V1,
            0,
        );
        xkb_context_unref(ctx);
        if keymap.is_null() {
            return None;
        }
        let table = table_from_keymap(keymap, group, locked);
        xkb_keymap_unref(keymap);
        Some(table)
    }
}

/// Index the keysyms `group` types, keeping for each the entry that needs
/// the fewest modifiers.
///
/// # Safety
/// `keymap` must be a valid keymap.
pub(super) unsafe fn table_from_keymap(
    keymap: *mut XkbKeymap,
    group: u32,
    locked: u32,
) -> KeyTable {
    let mut map = KeyTable::new();
    // SAFETY: the caller owns a live keymap. The state is released before return.
    unsafe {
        let state = xkb_state_new(keymap);
        if state.is_null() {
            return map;
        }
        let keys = xkb_keymap_min_keycode(keymap)..=xkb_keymap_max_keycode(keymap);
        // Discover actual modifier key actions instead of assuming that level N
        // always means Shift/AltGr. This also supports remapped and level-5 keys.
        xkb_state_update_mask(state, 0, 0, 0, 0, 0, group);
        let mut modifiers = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in keys.clone() {
            let sym = match xkb_state_key_get_one_sym(state, key) {
                0xffe2 => 0xffe1,
                sym => sym,
            };
            if matches!(sym, 0xffe1 | 0xffe2 | 0xfe03 | 0xfe11 | 0xff7e) && seen.insert(sym) {
                modifiers.push(key);
            }
        }
        // Duplicate left/right Shift adds no characters. Keep enumeration bounded
        // even with hostile or unusual compositor keymaps.
        modifiers.truncate(5);
        let mut combinations: Vec<Vec<u32>> = (0usize..(1 << modifiers.len()))
            .map(|mask| {
                modifiers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, key)| (mask & (1 << i) != 0).then_some(*key))
                    .collect()
            })
            .collect();
        combinations.sort_by_key(Vec::len);
        for mods in combinations {
            for key in keys
                .clone()
                .filter(|key| *key >= 9 && !modifiers.contains(key))
            {
                xkb_state_update_mask(state, 0, 0, locked, 0, 0, group);
                let baseline_locked = xkb_state_serialize_mods(state, 1 << 2);
                let baseline_group = xkb_state_serialize_layout(state, 1 << 7);
                for modifier in &mods {
                    xkb_state_update_key(state, *modifier, 1);
                }
                let sym = xkb_state_key_get_one_sym(state, key);
                for modifier in mods.iter().rev() {
                    xkb_state_update_key(state, *modifier, 0);
                }
                // A typing chord must not toggle a lock or leave another group active.
                if sym != 0
                    && xkb_state_serialize_mods(state, 1 << 2) == baseline_locked
                    && xkb_state_serialize_layout(state, 1 << 7) == baseline_group
                {
                    map.entry(sym).or_insert_with(|| KeycodeEntry {
                        evdev: key - 8,
                        mods: mods.iter().map(|key| key - 8).collect(),
                    });
                }
            }
        }
        xkb_state_unref(state);
    }
    map
}

// Text control characters need keyboard keysyms, not Unicode keysyms.
pub(super) fn text_keysym(ch: char) -> u32 {
    match ch {
        '\n' | '\r' => 0xff0d, // Return
        '\t' => 0xff09,        // Tab
        // SAFETY: a pure lookup without pointers.
        _ => unsafe { xkb_utf32_to_keysym(ch as u32) },
    }
}

pub(super) fn missing_characters(table: &KeyTable, text: &str) -> bool {
    text.chars().any(|ch| !table.contains_key(&text_keysym(ch)))
}
