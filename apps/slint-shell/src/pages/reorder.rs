//! Drag-and-drop reordering of list rows (`DragDrop` global).

use crate::ui::{DragDrop, SettingsWindow};
use slint::ComponentHandle;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Row positions by (group, index): start and size along the drag axis.
type RowGeometryMap = HashMap<(i32, i32), (f32, f32)>;

pub(super) fn bind(ui: &SettingsWindow) {
    let rows = Rc::new(RefCell::new(RowGeometryMap::new()));
    let positions = rows.clone();
    ui.global::<DragDrop>()
        .on_row(move |group, index, y, height| {
            positions.borrow_mut().insert((group, index), (y, height));
        });
    ui.global::<DragDrop>()
        .on_locate(move |group, source, offset, count| {
            let mut rows = rows.borrow_mut();
            // RowGeometryMap of a shrunk list never unregister; drop them so the map stays bounded.
            rows.retain(|&(g, index), _| g != group || index < count);
            locate(&rows, group, source, offset, count)
        });
}

/// Index the dragged row lands on: the farthest row whose middle its middle has passed.
fn locate(rows: &RowGeometryMap, group: i32, source: i32, offset: f32, count: i32) -> i32 {
    let Some(&(y, height)) = rows.get(&(group, source)) else {
        return source;
    };
    let center = y + height / 2.0 + offset;
    let mut target = source;
    for index in 0..count {
        if let Some(&(top, height)) = rows.get(&(group, index)) {
            if index > source && center >= top + height / 2.0 {
                target = target.max(index);
            } else if index < source && center <= top + height / 2.0 {
                target = target.min(index);
            }
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_follows_row_middles() {
        let rows: RowGeometryMap = (0..3).map(|i| ((1, i), (i as f32 * 40.0, 40.0))).collect();
        assert_eq!(locate(&rows, 1, 0, 0.0, 3), 0);
        assert_eq!(locate(&rows, 1, 0, 45.0, 3), 1);
        assert_eq!(locate(&rows, 1, 0, 200.0, 3), 2);
        assert_eq!(locate(&rows, 1, 2, -200.0, 3), 0);
        assert_eq!(
            locate(&rows, 1, 0, 200.0, 2),
            1,
            "rows past count are ignored"
        );
        assert_eq!(
            locate(&rows, 2, 0, 200.0, 3),
            0,
            "unknown group keeps the source"
        );
    }
}
