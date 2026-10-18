//! Launch on startup: an XDG autostart entry on Linux, a `Run` registry
//! value of the current user on Windows. Debug builds register in the
//! development directory instead, so they never change the real session.

#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::storage::APP_ID;
use crate::storage::StoragePaths;

/// Whether this OS can register the application to launch on startup.
pub const SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "windows"));

pub fn set_enabled(paths: &StoragePaths, enabled: bool) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        if !cfg!(debug_assertions) {
            return registry::configure(enabled, &executable);
        }
        configure(paths, enabled, &executable)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (paths, enabled);
        Err("Autostart registration is not implemented on this operating system".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
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
    let path = directory.join(format!(
        "{APP_ID}.{}",
        if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "desktop"
        }
    ));
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
    #[cfg(target_os = "windows")]
    let entry = registry::command_line(executable)?;
    #[cfg(target_os = "linux")]
    let entry = desktop_entry(executable)?;
    std::fs::create_dir_all(directory).map_err(|e| format!("create autostart directory: {e}"))?;
    crate::storage::write_atomic(&path, entry.as_bytes())
}

#[cfg(target_os = "linux")]
fn desktop_entry(executable: &str) -> Result<String, String> {
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
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Left Hand Control\nExec=\"{escaped}\"\nTerminal=false\n"
    ))
}

#[cfg(target_os = "windows")]
mod registry {
    use super::APP_ID;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
    };
    use windows::core::{HSTRING, w};

    /// The quoted executable; quotes keep a path with spaces one argument.
    pub(super) fn command_line(executable: &str) -> Result<String, String> {
        if executable.contains('"') {
            return Err("autostart executable path contains a quote".into());
        }
        Ok(format!("\"{executable}\""))
    }

    pub(super) fn configure(enabled: bool, executable: &std::path::Path) -> Result<(), String> {
        let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let name = HSTRING::from(APP_ID);
        if !enabled {
            let result = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key, &name) };
            return if result == ERROR_SUCCESS || result == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!(
                    "remove autostart entry: {}",
                    result.to_hresult().message()
                ))
            };
        }
        let executable = executable
            .to_str()
            .ok_or_else(|| "autostart executable path is not UTF-8".to_owned())?;
        let data: Vec<u16> = command_line(executable)?
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key,
                &name,
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * std::mem::size_of::<u16>()) as u32,
            )
        };
        if result == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!(
                "register autostart: {}",
                result.to_hresult().message()
            ))
        }
    }
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
