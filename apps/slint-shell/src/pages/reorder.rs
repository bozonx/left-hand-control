use crate::ui::{DragDrop, SettingsWindow};
use slint::ComponentHandle;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) fn bind(ui: &SettingsWindow) {
    let rows = Rc::new(RefCell::new(HashMap::new()));
    let positions = rows.clone();
    ui.global::<DragDrop>()
        .on_row(move |group, index, y, height| {
            positions.borrow_mut().insert((group, index), (y, height));
        });
    ui.global::<DragDrop>()
        .on_locate(move |group, source, offset, count| {
            let rows = rows.borrow();
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
        });
}
