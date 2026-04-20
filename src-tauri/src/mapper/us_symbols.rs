// Canonical list of every printable character the app can "type as a
// literal". Each entry lives on its own dedicated keycode in the Wayland
// virtual keyboard's custom keymap, so the physical system layout is
// irrelevant — pressing `sym+Q` always produces `~` whether the user is
// currently in English, Russian, German or Hebrew.
//
// The order of this table is load-bearing: the index of a character here
// is also the XKB-level keycode offset (0 → keycode 8, 1 → keycode 9, ...)
// that we send over the virtual-keyboard Wayland protocol. Never reorder
// rows once shipped; append-only.

#![cfg(target_os = "linux")]

/// One entry of the US symbol table.
///
///  - `ch`     — the Unicode character we expose to users (what appears in
///               the YAML config verbatim).
///  - `keysym` — the **XKB keysym name** that ends up in our generated
///               xkb_keymap. Naming follows `keysymdef.h` (e.g. `at`,
///               `asciitilde`, `braceleft`). Letters and digits use their
///               bare character form.
#[derive(Clone, Copy)]
pub struct SymEntry {
    pub ch: char,
    pub keysym: &'static str,
}

const fn e(ch: char, keysym: &'static str) -> SymEntry {
    SymEntry { ch, keysym }
}

/// Every printable character reachable on a US-QWERTY layout (unshifted
/// and shifted), plus space. 95 entries = the full ASCII printable range
/// 0x20..=0x7E.
///
/// The comments group entries by row of a physical keyboard for easier
/// human review.
pub const US_SYMBOLS: &[SymEntry] = &[
    // Space
    e(' ', "space"),

    // Digits row (unshifted) + top symbols (shifted)
    e('`', "grave"),
    e('1', "1"), e('2', "2"), e('3', "3"), e('4', "4"), e('5', "5"),
    e('6', "6"), e('7', "7"), e('8', "8"), e('9', "9"), e('0', "0"),
    e('-', "minus"), e('=', "equal"),
    e('~', "asciitilde"),
    e('!', "exclam"), e('@', "at"), e('#', "numbersign"), e('$', "dollar"),
    e('%', "percent"), e('^', "asciicircum"), e('&', "ampersand"),
    e('*', "asterisk"), e('(', "parenleft"), e(')', "parenright"),
    e('_', "underscore"), e('+', "plus"),

    // QWERTY row
    e('q', "q"), e('w', "w"), e('e', "e"), e('r', "r"), e('t', "t"),
    e('y', "y"), e('u', "u"), e('i', "i"), e('o', "o"), e('p', "p"),
    e('[', "bracketleft"), e(']', "bracketright"), e('\\', "backslash"),
    e('Q', "Q"), e('W', "W"), e('E', "E"), e('R', "R"), e('T', "T"),
    e('Y', "Y"), e('U', "U"), e('I', "I"), e('O', "O"), e('P', "P"),
    e('{', "braceleft"), e('}', "braceright"), e('|', "bar"),

    // ASDF row
    e('a', "a"), e('s', "s"), e('d', "d"), e('f', "f"), e('g', "g"),
    e('h', "h"), e('j', "j"), e('k', "k"), e('l', "l"),
    e(';', "semicolon"), e('\'', "apostrophe"),
    e('A', "A"), e('S', "S"), e('D', "D"), e('F', "F"), e('G', "G"),
    e('H', "H"), e('J', "J"), e('K', "K"), e('L', "L"),
    e(':', "colon"), e('"', "quotedbl"),

    // ZXCV row
    e('z', "z"), e('x', "x"), e('c', "c"), e('v', "v"), e('b', "b"),
    e('n', "n"), e('m', "m"),
    e(',', "comma"), e('.', "period"), e('/', "slash"),
    e('Z', "Z"), e('X', "X"), e('C', "C"), e('V', "V"), e('B', "B"),
    e('N', "N"), e('M', "M"),
    e('<', "less"), e('>', "greater"), e('?', "question"),
];

/// XKB keycodes start at 8 by convention (< 8 is reserved).
pub const KEYCODE_BASE: u32 = 8;

/// Evdev scancode to send over the virtual-keyboard protocol for `ch`,
/// or `None` if the character is not in our table. The scancode is the
/// XKB keycode minus 8, as required by `zwp_virtual_keyboard_v1.key`.
pub fn scancode_for(ch: char) -> Option<u32> {
    US_SYMBOLS.iter().position(|s| s.ch == ch).map(|i| i as u32)
}

/// Build an xkb_keymap string that assigns each entry in `US_SYMBOLS` to
/// its own one-level keycode. Fed to the compositor via the virtual
/// keyboard protocol so our key events are interpreted with *our* keymap,
/// not the user's system layout.
pub fn build_keymap_string() -> String {
    let max_keycode = KEYCODE_BASE + US_SYMBOLS.len() as u32 - 1;
    let mut s = String::with_capacity(8 * 1024);
    s.push_str("xkb_keymap {\n");

    // Keycodes section: declare every keycode we will ever send.
    s.push_str("xkb_keycodes \"lhc\" {\n");
    s.push_str(&format!(
        "    minimum = {};\n    maximum = {};\n",
        KEYCODE_BASE, max_keycode
    ));
    for (i, _) in US_SYMBOLS.iter().enumerate() {
        let kc = KEYCODE_BASE + i as u32;
        s.push_str(&format!("    <K{i}> = {kc};\n"));
    }
    s.push_str("};\n");

    // Types: a single one-level type so no modifiers are needed to
    // produce the keysym — the keycode by itself is enough.
    s.push_str("xkb_types \"lhc\" {\n");
    s.push_str("    virtual_modifiers LhcNone;\n");
    s.push_str("    type \"ONE_LEVEL\" {\n");
    s.push_str("        modifiers = none;\n");
    s.push_str("        level_name[Level1] = \"Any\";\n");
    s.push_str("    };\n");
    s.push_str("};\n");

    // Compatibility: empty but required.
    s.push_str("xkb_compatibility \"lhc\" {};\n");

    // Symbols: map each keycode to its keysym at level 1.
    s.push_str("xkb_symbols \"lhc\" {\n");
    s.push_str("    name[Group1] = \"LeftHandControl\";\n");
    for (i, sym) in US_SYMBOLS.iter().enumerate() {
        s.push_str(&format!(
            "    key <K{i}> {{ type = \"ONE_LEVEL\", [ {sym} ] }};\n",
            i = i,
            sym = sym.keysym
        ));
    }
    s.push_str("};\n");

    s.push_str("};\n");
    s
}
