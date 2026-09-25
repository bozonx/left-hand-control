//! Random ids for new entities, like the frontend's `genId()`.

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
