use slint::ComponentHandle;
slint::slint! {
    export component Fixture inherits Window {
        title: "Slint window fixture";
        preferred-width: 800px; preferred-height: 600px;
        min-width: 100px; min-height: 100px;
        Text { text: "Window fixture"; }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = Fixture::new()?;
    match std::env::var("SLINT_FIXTURE_MODE").as_deref() {
        Ok("fullscreen") => ui.window().set_fullscreen(true),
        Ok("maximized") => ui.window().set_maximized(true),
        _ => {}
    }
    ui.show()?;
    slint::run_event_loop()?;
    Ok(())
}
