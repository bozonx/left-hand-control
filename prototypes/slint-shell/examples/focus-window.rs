use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid: u32 = std::env::args()
        .nth(1)
        .ok_or("expected a window process ID")?
        .parse()?;
    let mut script = tempfile::NamedTempFile::new()?;
    write!(
        script,
        "let pid = {pid}; for (let w of workspace.windowList()) {{ if (w.pid === pid) {{ workspace.activeWindow = w; break; }} }}"
    )?;
    let connection = zbus::blocking::Connection::session()?;
    let scripting = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;
    let name = format!("lhc-focus-window-{}", std::process::id());
    let id: i32 = scripting.call(
        "loadScript",
        &(script.path().to_string_lossy().as_ref(), &name),
    )?;
    if id < 0 {
        return Err("KWin focus script failed to load".into());
    }
    let run = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        "org.kde.kwin.Script",
    )?;
    let result = run.call::<_, _, ()>("run", &());
    let _: bool = scripting.call("unloadScript", &name)?;
    result?;
    Ok(())
}
