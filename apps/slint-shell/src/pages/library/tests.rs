//! Regression tests for library.

use super::*;

#[test]
fn dragging_uses_actual_card_heights() {
    let rows = [Some((0.0, 200.0)), Some((200.0, 96.0)), Some((296.0, 96.0))];
    assert_eq!(drag_target(&rows, 0, 244.0, 3), 2);
    assert_eq!(drag_target(&rows, 2, -244.0, 3), 0);
    assert_eq!(drag_target(&rows, 0, 244.0, 2), 1);
}

#[test]
fn conditions_round_trip() {
    let set = condition(GameCondition::On, "us, ru", "");
    assert_eq!(set.layouts, ["us", "ru"]);
    assert_eq!(
        condition_fields(Some(set.clone())),
        (GameCondition::On, "us, ru".into(), String::new())
    );
    assert_eq!(summary(&set), (GameCondition::On, "us, ru".into()));
    assert!(condition(GameCondition::Any, " ", "").is_empty());
}
