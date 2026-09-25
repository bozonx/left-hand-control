use slint::SharedString;

pub fn emoji_items() -> Vec<SharedString> {
    (0x1f600..=0x1f64f)
        .chain(0x1f300..=0x1f5ff)
        .filter_map(char::from_u32)
        .take(240)
        .map(|character| character.to_string().into())
        .collect()
}

pub fn quick_items() -> Vec<String> {
    (1..=30)
        .map(|index| format!("Действие {index:02} / Action {index:02}"))
        .collect()
}

pub fn filter(items: &[String], query: &str) -> Vec<SharedString> {
    let query = query.to_lowercase();
    items
        .iter()
        .filter(|item| item.to_lowercase().contains(&query))
        .map(|item| item.as_str().into())
        .collect()
}

pub fn emoji_index(page: i32, selected: i32) -> Option<usize> {
    if page < 0 || selected < 0 || selected >= if page == 5 { 1500 } else { 48 } {
        return None;
    }
    usize::try_from(if page == 5 {
        selected
    } else {
        page.checked_mul(48)?.checked_add(selected)?
    })
    .ok()
}

pub fn advance(selected: i32, delta: i32, count: usize) -> i32 {
    if count == 0 {
        0
    } else {
        (selected + delta).rem_euclid(count as i32)
    }
}

pub fn key_delta(name: &str, key: &str) -> Option<i32> {
    let is = |expected: slint::platform::Key| key == SharedString::from(expected).as_str();
    if is(slint::platform::Key::DownArrow) {
        Some(if name == "emoji" { 8 } else { 1 })
    } else if is(slint::platform::Key::UpArrow) {
        Some(if name == "emoji" { -8 } else { -1 })
    } else if name == "emoji" && is(slint::platform::Key::LeftArrow) {
        Some(-1)
    } else if name == "emoji" && is(slint::platform::Key::RightArrow) {
        Some(1)
    } else {
        None
    }
}
