//! Helpers shared by standalone desktop scenarios.
#![allow(dead_code)] // Each example uses a different subset of the helpers.

use lhc_core::storage::StoragePaths;
use slint::ComponentHandle;
use slint_shell::{Document, bind_document, ui::SettingsWindow};
use std::{
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub paths: StoragePaths,
    pub document: Rc<Document>,
    pub ui: SettingsWindow,
}

impl Fixture {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let document = Document::load(paths.clone())?;
        let ui = SettingsWindow::new()?;
        bind_document(&ui, &document);
        Ok(Self {
            dir,
            paths,
            document,
            ui,
        })
    }
}

pub fn wait_until(description: &str, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        slint::platform::update_timers_and_animations();
        if ready() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn write_snapshot(ui: &impl ComponentHandle, path: &Path) {
    let pixels = ui.window().take_snapshot().unwrap();
    let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
    for pixel in pixels.as_slice() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
    }
    std::fs::write(path, bytes).unwrap();
}

pub fn snapshot(ui: &impl ComponentHandle, name: &str) {
    if let Some(dir) = std::env::var_os("LHC_EDITOR_SNAPSHOTS") {
        write_snapshot(
            ui,
            &std::path::PathBuf::from(dir).join(format!("{name}.ppm")),
        );
    }
}

pub fn quick_action(
    layout: &lhc_core::profile::model::LayoutPreset,
    page: usize,
    key: &str,
) -> String {
    layout.quick_action_pages[page]
        .cells
        .get(key)
        .map(|item| item.action.clone())
        .unwrap_or_default()
}

/// Resolve the actual visible control bounds instead of assuming font metrics.
pub fn accessible_position(
    ui: &SettingsWindow,
    role: i_slint_core::items::AccessibleRole,
    label: &str,
    prefix: bool,
) -> slint::LogicalPosition {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, window::WindowInner,
    };
    let _ = ui.window().take_snapshot().unwrap();
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    let mut matches = Vec::new();
    while let Some(item) = pending.pop() {
        if item.is_visible()
            && item.accessible_role() == role
            && item
                .accessible_string_property(AccessibleStringProperty::Label)
                .as_deref()
                .is_some_and(|value| {
                    if prefix {
                        value.starts_with(label)
                    } else {
                        value == label
                    }
                })
        {
            let center = item.map_to_window(item.geometry().center());
            matches.push(slint::LogicalPosition::new(center.x, center.y));
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    assert_eq!(matches.len(), 1, "expected one visible {role:?}: {label}");
    matches[0]
}
