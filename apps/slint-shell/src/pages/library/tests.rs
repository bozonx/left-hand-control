//! Regression tests for library.

use super::*;

#[test]
fn conditions_round_trip() {
    let set = condition(GameCondition::On, "us, ru", "", "kate");
    assert_eq!(set.layouts, ["us", "ru"]);
    assert_eq!(set.excluded_window_patterns, ["kate"]);
    assert_eq!(
        condition_fields(Some(set.clone())),
        (
            GameCondition::On,
            "us, ru".into(),
            String::new(),
            "kate".into()
        )
    );
    assert_eq!(summary(&set), (GameCondition::On, "us, ru".into()));
    assert!(condition(GameCondition::Any, " ", "", " ").is_empty());
}
