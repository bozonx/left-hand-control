//! Launch on startup: an XDG autostart entry on Linux, a `Run` registry
//! value of the current user on Windows. Debug builds and `LHC_DEV_DIR`
//! register in the development directory instead, so they never change the
//! real session.
//!
//! The operating system owns the registration: the user can also remove or
//! disable it in the desktop's autostart settings (KDE, GNOME Tweaks) or in
//! the Windows Task Manager. [`is_enabled`] reports that state so the
//! setting can follow it.

#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::storage::APP_ID;
use crate::storage::StoragePaths;

/// Whether this OS can register the application to launch on startup.
pub const SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "windows"));

/// Register or remove the launch on startup. Registering again refreshes the
/// executable path, e.g. after the application was moved or upgraded.
pub fn set_enabled(paths: &StoragePaths, enabled: bool) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        #[cfg(target_os = "windows")]
        if !crate::storage::isolated() {
            return registry::configure(enabled);
        }
        configure(paths, enabled)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (paths, enabled);
        Err("Autostart registration is not implemented on this operating system".into())
    }
}

/// Whether the OS will launch the application at the next login.
pub fn is_enabled(paths: &StoragePaths) -> Result<bool, String> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        #[cfg(target_os = "windows")]
        if !crate::storage::isolated() {
            return registry::is_enabled();
        }
        let text = match std::fs::read_to_string(entry_path(paths)?) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("read autostart entry: {error}")),
        };
        Ok(cfg!(target_os = "windows") || entry_active(&text))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = paths;
        Ok(false)
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn entry_path(paths: &StoragePaths) -> Result<std::path::PathBuf, String> {
    let directory = paths
        .settings_dir()
        .parent()
        .ok_or_else(|| "resolve autostart directory".to_owned())?
        .join("autostart");
    let extension = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "desktop"
    };
    Ok(directory.join(format!("{APP_ID}.{extension}")))
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn configure(paths: &StoragePaths, enabled: bool) -> Result<(), String> {
    let path = entry_path(paths)?;
    if !enabled {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("remove autostart entry: {error}")),
        };
    }
    let executable = launch_path()?;
    let executable = executable
        .to_str()
        .ok_or_else(|| "autostart executable path is not UTF-8".to_owned())?;
    #[cfg(target_os = "windows")]
    let entry = registry::command_line(executable)?;
    #[cfg(target_os = "linux")]
    let entry = desktop_entry(executable)?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|e| format!("create autostart directory: {e}"))?;
    }
    crate::storage::write_atomic(&path, entry.as_bytes())
}

/// The executable to launch at login.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn launch_path() -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    let executable = stable_path(executable);
    Ok(executable)
}

/// A path that keeps working after package upgrades. Linux reports a
/// replaced binary as `<path> (deleted)`, and a `PATH` entry that resolves to
/// this binary (`/usr/bin`, `~/.local/bin`, a NixOS profile) outlives a
/// versioned target such as a Nix store path.
#[cfg(target_os = "linux")]
fn stable_path(executable: std::path::PathBuf) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let bytes = executable.as_os_str().as_bytes();
    let executable = match bytes.strip_suffix(b" (deleted)") {
        Some(path) => std::path::PathBuf::from(std::ffi::OsStr::from_bytes(path)),
        None => executable,
    };
    let (Some(name), Ok(target), Some(search)) = (
        executable.file_name(),
        executable.canonicalize(),
        std::env::var_os("PATH"),
    ) else {
        return executable;
    };
    std::env::split_paths(&search)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            candidate.is_absolute() && candidate.canonicalize().is_ok_and(|path| path == target)
        })
        .unwrap_or(executable)
}

/// Whether a desktop entry is enabled: the desktop's autostart settings may
/// keep a disabled entry with `Hidden=true` or, on GNOME,
/// `X-GNOME-Autostart-enabled=false`.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn entry_active(text: &str) -> bool {
    let mut main_group = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            main_group = line == "[Desktop Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Hidden" if main_group && value == "true" => return false,
            "X-GNOME-Autostart-enabled" if main_group && value == "false" => return false,
            _ => {}
        }
    }
    true
}

#[cfg(target_os = "linux")]
fn desktop_entry(executable: &str) -> Result<String, String> {
    if executable.contains(['\n', '\r']) {
        return Err("autostart executable path contains a newline".into());
    }
    // Exec quotes the path as one argument; TryExec is a plain string. With
    // TryExec, desktops skip the entry once the binary is uninstalled.
    let mut exec = String::new();
    for character in executable.chars() {
        match character {
            '%' => exec.push_str("%%"),
            '\\' => exec.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                exec.push_str("\\\\");
                exec.push(character);
            }
            other => exec.push(other),
        }
    }
    let try_exec = executable.replace('\\', "\\\\");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Left Hand Control\n\
         Icon={APP_ID}\nExec=\"{exec}\"\nTryExec={try_exec}\nTerminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    ))
}

#[cfg(target_os = "windows")]
mod registry {
    use super::APP_ID;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW,
        RegGetValueW, RegSetKeyValueW,
    };
    use windows::core::{HSTRING, PCWSTR, w};

    const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    /// Where Task Manager's "Startup apps" records that the user disabled a
    /// `Run` entry; an odd first byte means disabled.
    const APPROVED: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");

    /// The quoted executable; quotes keep a path with spaces one argument.
    pub(super) fn command_line(executable: &str) -> Result<String, String> {
        if executable.contains('"') {
            return Err("autostart executable path contains a quote".into());
        }
        Ok(format!("\"{executable}\""))
    }

    pub(super) fn is_enabled() -> Result<bool, String> {
        let name = HSTRING::from(APP_ID);
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN,
                &name,
                RRF_RT_REG_SZ,
                None,
                None,
                None,
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if result != ERROR_SUCCESS {
            return Err(format!(
                "read autostart entry: {}",
                result.to_hresult().message()
            ));
        }
        let mut data = [0u8; 64];
        let mut size = data.len() as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                APPROVED,
                &name,
                RRF_RT_REG_BINARY,
                None,
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        Ok(if result == ERROR_SUCCESS && size > 0 {
            data[0] & 1 == 0
        } else {
            result == ERROR_FILE_NOT_FOUND || result == ERROR_MORE_DATA
        })
    }

    fn delete(key: PCWSTR, name: &HSTRING) -> Result<(), String> {
        let result = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key, name) };
        if result == ERROR_SUCCESS || result == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!(
                "remove autostart entry: {}",
                result.to_hresult().message()
            ))
        }
    }

    pub(super) fn configure(enabled: bool) -> Result<(), String> {
        let name = HSTRING::from(APP_ID);
        // Enabling from the application overrides a Task Manager "disabled";
        // without the marker Windows treats the entry as enabled.
        delete(APPROVED, &name)?;
        if !enabled {
            return delete(RUN, &name);
        }
        let executable = super::launch_path()?;
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
                RUN,
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
    fn desktop_entry_escapes_the_executable() {
        let text = desktop_entry("/opt/a b/$test%/back\\slash/left-hand-control").unwrap();
        assert!(text.contains("Exec=\"/opt/a b/\\\\$test%%/back\\\\\\\\slash/left-hand-control\""));
        assert!(text.contains("TryExec=/opt/a b/$test%/back\\\\slash/left-hand-control\n"));
        assert!(entry_active(&text));
        assert!(desktop_entry("/opt/a\nb").is_err());
    }

    #[test]
    fn registers_and_removes_only_its_own_entry() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config/app"), dir.path().join("data"));
        assert!(!is_enabled(&paths).unwrap());
        configure(&paths, true).unwrap();
        assert!(is_enabled(&paths).unwrap());
        let directory = dir.path().join("config/autostart");
        let file = directory.join(format!("{APP_ID}.desktop"));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("Exec=\"/"));
        std::fs::write(directory.join("other.desktop"), "other").unwrap();
        configure(&paths, false).unwrap();
        assert!(!file.exists());
        assert!(!is_enabled(&paths).unwrap());
        assert!(directory.join("other.desktop").exists());
        configure(&paths, false).unwrap();
    }

    #[test]
    fn entries_disabled_by_the_desktop_are_not_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config/app"), dir.path().join("data"));
        let file = entry_path(&paths).unwrap();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        for (text, enabled) in [
            ("[Desktop Entry]\nExec=lhc\n", true),
            ("[Desktop Entry]\nExec=lhc\nHidden=true\n", false),
            ("[Desktop Entry]\nX-GNOME-Autostart-enabled=false\n", false),
            ("[Desktop Entry]\nX-GNOME-Autostart-enabled=true\n", true),
            ("[Desktop Entry]\n[Desktop Action x]\nHidden=true\n", true),
        ] {
            std::fs::write(&file, text).unwrap();
            assert_eq!(is_enabled(&paths).unwrap(), enabled, "{text}");
        }
    }

    #[test]
    fn stable_path_drops_the_deleted_suffix() {
        let path = stable_path("/nonexistent/lhc-test-binary (deleted)".into());
        assert_eq!(path, std::path::Path::new("/nonexistent/lhc-test-binary"));
    }
}
