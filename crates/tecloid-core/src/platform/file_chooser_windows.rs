//! Native file selection on Windows through the common item dialog.

use std::path::PathBuf;
use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
};
use windows::core::HSTRING;

/// Ask for a file (or a directory); `Ok(None)` when the user cancels.
/// Blocks until the dialog closes, so call it off the UI thread.
pub fn pick(title: &str, directory: bool) -> Result<Option<PathBuf>, String> {
    // The dialog needs a single-threaded apartment on this thread.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = show(title, directory);
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn show(title: &str, directory: bool) -> Result<Option<PathBuf>, String> {
    let error = |error: windows::core::Error| format!("File dialog failed: {}", error.message());
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }.map_err(error)?;
    unsafe {
        let mut options = dialog.GetOptions().map_err(error)? | FOS_FORCEFILESYSTEM;
        if directory {
            options |= FOS_PICKFOLDERS;
        }
        dialog.SetOptions(options).map_err(error)?;
        dialog.SetTitle(&HSTRING::from(title)).map_err(error)?;
        if let Err(shown) = dialog.Show(None) {
            if shown.code() == ERROR_CANCELLED.to_hresult() {
                return Ok(None);
            }
            return Err(error(shown));
        }
        let item = dialog.GetResult().map_err(error)?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(error)?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(path.map_err(|e| e.to_string())?)))
    }
}
