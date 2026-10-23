//! Random ids for new configuration entities.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// `prefix` followed by eight base-36 characters.
pub fn generate(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut value = hasher.finish();
    let mut out = String::from(prefix);
    for _ in 0..8 {
        out.push(char::from_digit((value % 36) as u32, 36).unwrap_or('0'));
        value /= 36;
    }
    out
}

/// Validate identity within one collection. Different entity kinds may share ids.
pub(crate) fn unique<'a>(ids: impl IntoIterator<Item = &'a str>, path: &str) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for (index, id) in ids.into_iter().enumerate() {
        if id.trim().is_empty() || id.trim() != id {
            return Err(format!(
                "{path}[{index}].id: expected a non-empty id without surrounding whitespace"
            ));
        }
        if !seen.insert(id) {
            return Err(format!("{path}[{index}].id: duplicate id \"{id}\""));
        }
    }
    Ok(())
}

/// Fill absent ids deterministically, reserving every explicit id first.
pub(crate) fn complete(ids: &mut [Option<String>], prefix: &str, path: &str) -> Result<(), String> {
    let mut reserved = std::collections::HashSet::new();
    for (index, id) in ids.iter().enumerate() {
        if let Some(id) = id {
            unique([id.as_str()], &format!("{path}[{index}]"))?;
            if !reserved.insert(id.clone()) {
                return Err(format!("{path}[{index}].id: duplicate id \"{id}\""));
            }
        }
    }
    let mut counter = 0u64;
    for id in ids.iter_mut().filter(|id| id.is_none()) {
        loop {
            counter += 1;
            let candidate = format!("{prefix}{counter}");
            if reserved.insert(candidate.clone()) {
                *id = Some(candidate);
                break;
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_layout(layout: &super::model::LayoutPreset) -> Result<(), String> {
    unique(layout.layers.iter().map(|item| item.id.as_str()), "layers")?;
    unique(layout.rules.iter().map(|item| item.id.as_str()), "rules")?;
    unique(layout.macros.iter().map(|item| item.id.as_str()), "macros")?;
    unique(
        layout.commands.iter().map(|item| item.id.as_str()),
        "commands",
    )?;
    unique(
        layout
            .quick_action_pages
            .iter()
            .map(|item| item.id.as_str()),
        "quickActionPages",
    )?;
    unique(
        layout.emoji_pages.iter().map(|item| item.id.as_str()),
        "emojiPages",
    )?;
    for item in &layout.macros {
        unique(
            item.steps.iter().map(|step| step.id.as_str()),
            &format!("macros[{}].steps", item.id),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_prefixed_and_distinct() {
        let a = super::generate("r_");
        let b = super::generate("r_");
        assert!(a.starts_with("r_") && a.len() == 10);
        assert_ne!(a, b);
    }
}
