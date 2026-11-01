//! Key editor stand. `--smoke` only checks that the keyboard page renders in a
//! real window and exits; its callbacks are covered by `tests/page_callbacks.rs`.

mod support;
use lhc_core::storage::StoragePaths;
use slint::ComponentHandle;
use slint_shell::{Document, bind_document, ui::*};
use support::snapshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.set_page(Page::Keyboard);
    if std::env::args().any(|arg| arg == "--smoke") {
        ui.show()?;
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
            let ui = weak.unwrap();
            snapshot(&ui, "keyboard");
            println!("Editor smoke: passed (the keyboard page renders in a real window)");
            slint::quit_event_loop().unwrap();
        });
    }
    ui.run()?;
    Ok(())
}
