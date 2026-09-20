use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Report(Arc<Mutex<Option<String>>>);
#[zbus::interface(name = "org.leftHandControl.GeometryProbe")]
impl Report {
    fn report(&self, json: String) {
        *self.0.lock().unwrap() = Some(json);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pids = std::env::args()
        .skip(1)
        .map(|s| s.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()?;
    if pids.is_empty() {
        return Err("expected prototype process IDs".into());
    }
    let pids = serde_json::to_string(&pids)?;
    let value = Arc::new(Mutex::new(None));
    let name = format!("org.leftHandControl.GeometryProbe.p{}", std::process::id());
    let connection = zbus::blocking::connection::Builder::session()?
        .name(name.clone())?
        .serve_at("/Probe", Report(value.clone()))?
        .build()?;
    let mut script = tempfile::NamedTempFile::new()?;
    write!(
        script,
        "const rows = []; for (let w of workspace.windowList()) {{ const app = String(w.resourceClass); if (!{pids}.includes(w.pid)) continue; const r = w.frameGeometry; const a = workspace.clientArea(KWin.PlacementArea, w); rows.push({{app: app, pid:w.pid, stack:workspace.stackingOrder.indexOf(w), skipTaskbar:w.skipTaskbar, noBorder:w.noBorder, clientWidth:w.clientGeometry.width, clientHeight:w.clientGeometry.height, fullscreen:w.fullScreen, x:r.x, y:r.y, width:r.width, height:r.height, output:w.output.name, area:{{x:a.x,y:a.y,width:a.width,height:a.height}}}}); }} callDBus('{name}', '/Probe', 'org.leftHandControl.GeometryProbe', 'Report', JSON.stringify(rows));"
    )?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;
    let script_name = format!("lhc-geometry-{}", std::process::id());
    let id: i32 = proxy.call(
        "loadScript",
        &(script.path().to_string_lossy().as_ref(), &script_name),
    )?;
    if id < 0 {
        return Err("geometry script failed to load".into());
    }
    let run = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        "org.kde.kwin.Script",
    )?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        run.call::<_, _, ()>("run", &())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(json) = value.lock().unwrap().take() {
                println!("{json}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("geometry report timed out".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    let _: bool = proxy.call("unloadScript", &script_name)?;
    result
}
