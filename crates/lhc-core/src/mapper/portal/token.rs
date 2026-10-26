//! Persistence of the portal restore tokens.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum TokenScope {
    Keyboard,
    Clipboard,
}

impl TokenScope {
    pub(super) fn filename(self) -> &'static str {
        match self {
            Self::Keyboard => RESTORE_TOKEN_FILE,
            Self::Clipboard => CLIPBOARD_RESTORE_TOKEN_FILE,
        }
    }
}

pub(super) fn restore_token_path(scope: TokenScope) -> Option<PathBuf> {
    TOKEN_DIR.get().map(|d| d.join(scope.filename()))
}

/// In-memory cache so we don't hit the filesystem on every handshake
/// (handshake is rare, but reads are cheap and this also gives us a sane
/// fallback when the disk path was not registered yet).
// A keyboard-only grant cannot authorize a newly requested clipboard, and a
// clipboard grant must not leak into pure-keyboard modes on session restore.
pub(super) static TOKEN_CACHE: Mutex<[Option<String>; 2]> = Mutex::new([None, None]);

pub(super) fn load_restore_token(scope: TokenScope) -> Option<String> {
    if let Ok(guard) = TOKEN_CACHE.lock()
        && let Some(cached) = guard[scope as usize].clone()
    {
        return Some(cached);
    }
    let path = restore_token_path(scope)?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let token = trimmed.to_string();
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        guard[scope as usize] = Some(token.clone());
    }
    Some(token)
}

pub(super) fn persist_restore_token(scope: TokenScope, token: &str) {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        guard[scope as usize] = Some(token.to_string());
    }
    let Some(path) = restore_token_path(scope) else {
        log::debug!("[portal] cannot persist restore_token (TOKEN_DIR not set)");
        return;
    };
    if let Err(e) = write_token_atomic(&path, token) {
        log::debug!("[portal] failed to persist restore_token: {e}");
    } else {
        log::debug!("[portal] restore_token saved to {}", path.display());
    }
}

pub(super) fn clear_restore_token(scope: TokenScope) {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        guard[scope as usize] = None;
    }
    if let Some(path) = restore_token_path(scope) {
        let _ = std::fs::remove_file(path);
    }
}

pub(super) fn write_token_atomic(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("token.tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
