//! Pseudo-translation for layout checks in debug builds: every string becomes
//! about 40% longer with accented letters, e.g. `[Ŝéţţîñĝš ~~~]`. It is bundled
//! as language `qps-ploc`, which is not offered in the app's language list.

pub const CODE: &str = "qps-ploc";

/// Placeholders (`{}`, `{0}`, `{n}`) stay intact so formatting still works.
pub fn pseudo(source: &str) -> String {
    let mut out = String::from("[");
    let mut letters = 0;
    let mut placeholder = false;
    for c in source.chars() {
        placeholder |= c == '{';
        let keep = placeholder;
        placeholder &= c != '}';
        out.push(if keep {
            c
        } else {
            letters += usize::from(c.is_alphanumeric());
            match c {
                'a' => 'á',
                'c' => 'ç',
                'e' => 'é',
                'g' => 'ĝ',
                'i' => 'î',
                'n' => 'ñ',
                'o' => 'ö',
                's' => 'š',
                't' => 'ţ',
                'u' => 'û',
                'y' => 'ý',
                'z' => 'ž',
                'A' => 'Å',
                'C' => 'Ç',
                'E' => 'É',
                'O' => 'Ö',
                'S' => 'Ŝ',
                'U' => 'Û',
                other => other,
            }
        });
    }
    if letters > 0 {
        out.push(' ');
        out.extend(std::iter::repeat_n('~', (letters * 2).div_ceil(5)));
    }
    out.push(']');
    out
}

fn quote(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

/// A PO catalog with the same entries as `catalog`, translated with [`pseudo`].
pub fn catalog(catalog: &rspolib::POFile) -> String {
    let mut po = format!(
        "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\"Language: {CODE}\\n\"\n\"Plural-Forms: nplurals=2; plural=(n != 1);\\n\"\n"
    );
    for entry in catalog.entries.iter().filter(|entry| !entry.obsolete) {
        po.push('\n');
        if let Some(context) = &entry.msgctxt {
            po.push_str(&format!("msgctxt {}\n", quote(context)));
        }
        po.push_str(&format!("msgid {}\n", quote(&entry.msgid)));
        if let Some(plural) = &entry.msgid_plural {
            po.push_str(&format!("msgid_plural {}\n", quote(plural)));
            po.push_str(&format!("msgstr[0] {}\n", quote(&pseudo(&entry.msgid))));
            po.push_str(&format!("msgstr[1] {}\n", quote(&pseudo(plural))));
        } else {
            po.push_str(&format!("msgstr {}\n", quote(&pseudo(&entry.msgid))));
        }
    }
    po
}
