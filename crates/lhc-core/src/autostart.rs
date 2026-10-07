#[cfg(target_os = "linux")]
use crate::storage::APP_ID;
use crate::storage::StoragePaths;

pub fn set_enabled(paths: &StoragePaths, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        configure(
            paths,
            enabled,
            &std::env::current_exe().map_err(|e| e.to_string())?,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (paths, enabled);
        Err("Autostart registration is not implemented on this operating system".into())
    }
}

#[cfg(target_os = "linux")]
fn configure(
    paths: &StoragePaths,
    enabled: bool,
    executable: &std::path::Path,
) -> Result<(), String> {
    let directory = paths
        .settings_dir()
        .parent()
        .ok_or_else(|| "resolve autostart directory".to_owned())?
        .join("autostart");
    let path = directory.join(format!("{APP_ID}.desktop"));
    if !enabled {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("remove autostart entry: {error}")),
        };
    }
    let executable = executable
        .to_str()
        .ok_or_else(|| "autostart executable path is not UTF-8".to_owned())?;
    let mut escaped = String::new();
    for character in executable.chars() {
        match character {
            '%' => escaped.push_str("%%"),
            '\\' => escaped.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                escaped.push_str("\\\\");
                escaped.push(character);
            }
            '\n' | '\r' => return Err("autostart executable path contains a newline".into()),
            other => escaped.push(other),
        }
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Left Hand Control\nExec=\"{escaped}\"\nTerminal=false\n"
    );
    std::fs::create_dir_all(directory).map_err(|e| format!("create autostart directory: {e}"))?;
    crate::storage::write_atomic(&path, entry.as_bytes())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn registers_escaped_executable_and_removes_only_its_own_entry() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config/app"), dir.path().join("data"));
        configure(
            &paths,
            true,
            std::path::Path::new("/opt/a b/$test%/slint-shell"),
        )
        .unwrap();
        let directory = dir.path().join("config/autostart");
        let file = directory.join(format!("{APP_ID}.desktop"));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("Exec=\"/opt/a b/\\\\$test%%/slint-shell\""));
        std::fs::write(directory.join("other.desktop"), "other").unwrap();
        configure(&paths, false, std::path::Path::new("/unused")).unwrap();
        assert!(!file.exists());
        assert!(directory.join("other.desktop").exists());
        configure(&paths, false, std::path::Path::new("/unused")).unwrap();
    }
}
