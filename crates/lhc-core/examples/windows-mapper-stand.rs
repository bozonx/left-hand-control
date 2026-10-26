//! Local VM acceptance driver. The file mailbox is test-only, not application IPC.
//! Run in an interactive disposable Windows session; input is session-wide.

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("windows-mapper-stand requires Windows");
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if lhc_core::mapper::windows::dispatch_input_guard()? {
        return Ok(());
    }
    use lhc_core::mapper::{runtime, windows::KEYBOARD_PATH};
    use serde_json::{Value, json};
    use std::{
        fs,
        path::PathBuf,
        thread,
        time::{Duration, Instant},
    };

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = runtime::stop();
            lhc_core::active_window::stop_watcher();
            lhc_core::keyboard_layout::stop_watcher();
        }
    }
    let argument = std::env::args_os()
        .nth(1)
        .ok_or("mailbox directory required")?;
    if argument == "--probe-lock" {
        return match runtime::start(KEYBOARD_PATH, None, "{}") {
            Err(error) if error.contains("already active in another process") => Ok(()),
            other => {
                let _ = runtime::stop();
                Err(format!("Expected mapper lock rejection, got {other:?}").into())
            }
        };
    }
    let directory = PathBuf::from(argument);
    fs::create_dir_all(&directory)?;
    let _cleanup = Cleanup;
    lhc_core::active_window::start_watcher();
    lhc_core::keyboard_layout::start_watcher();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    lhc_core::events::bus().subscribe(move |event| {
        let _ = event_tx.send(format!("{event:?}"));
    });
    fs::write(directory.join("ready"), std::process::id().to_string())?;
    let mut last_id = Value::Null;
    struct CrashProbe(Option<std::process::Child>);
    impl Drop for CrashProbe {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let mut crash_probe = CrashProbe(None);
    let mut last_request = Instant::now();
    while last_request.elapsed() < Duration::from_secs(600) {
        let request = fs::read(directory.join("request.json"))
            .ok()
            .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok());
        let Some(request) = request else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        if request["id"] == last_id {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        last_id = request["id"].clone();
        last_request = Instant::now();
        let op = request["op"].as_str().unwrap_or_default();
        let result = match op {
            "start" => runtime::start(KEYBOARD_PATH, None, &request["config"].to_string()),
            "config" => runtime::update_config(&request["config"].to_string()),
            "stop" => runtime::stop(),
            "crash-start" => {
                use std::os::windows::process::CommandExt;
                if crash_probe.0.is_some() {
                    Err("Crash probe already exists".into())
                } else {
                    let probe_directory = directory.join("crash");
                    if probe_directory.exists() {
                        fs::remove_dir_all(&probe_directory)?;
                    }
                    crash_probe.0 = Some(
                        std::process::Command::new(std::env::current_exe()?)
                            .arg(probe_directory)
                            .creation_flags(0x0800_0000)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()?,
                    );
                    Ok(())
                }
            }
            "crash-kill" => match crash_probe.0.take() {
                Some(mut child) => {
                    child.kill()?;
                    child.wait()?;
                    Ok(())
                }
                None => Err("No crash probe".into()),
            },
            "execute" => {
                runtime::execute_action(request["action"].as_str().unwrap_or_default().into())
            }
            "lock-probe" => {
                use std::os::windows::process::CommandExt;
                let status = std::process::Command::new(std::env::current_exe()?)
                    .arg("--probe-lock")
                    .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
                    .status()?;
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("lock probe: {status}"))
                }
            }
            "status" | "shutdown" => Ok(()),
            _ => Err(format!("Unknown stand operation: {op}")),
        };
        // Observe asynchronous control processing before answering the host.
        thread::sleep(Duration::from_millis(100));
        let response = json!({
            "id": last_id, "ok": result.is_ok(), "error": result.err(),
            "guard_pid": lhc_core::mapper::windows::input_guard_pid(),
            "status": runtime::status(), "events": event_rx.try_iter().collect::<Vec<_>>(),
        });
        fs::write(
            directory.join("response.tmp"),
            serde_json::to_vec(&response)?,
        )?;
        fs::rename(
            directory.join("response.tmp"),
            directory.join("response.json"),
        )?;
        if op == "shutdown" {
            return Ok(());
        }
    }
    Err("host command timeout".into())
}
