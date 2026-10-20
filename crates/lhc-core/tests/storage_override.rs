//! Verify storage overrides in a child process, including release test builds.

use lhc_core::storage::StoragePaths;
use std::process::Command;

#[test]
fn explicit_storage_override_is_resolved_in_a_fresh_process() {
    let directory = tempfile::tempdir().unwrap();
    for override_dir in [directory.path().join("absolute"), "relative".into()] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "storage_override_probe", "--nocapture"])
            .current_dir(directory.path())
            .env("LHC_STORAGE_PROBE", "1")
            .env("LHC_DEV_DIR", &override_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn storage_override_probe() {
    if std::env::var_os("LHC_STORAGE_PROBE").is_none() {
        return;
    }
    let base = std::path::PathBuf::from(std::env::var_os("LHC_DEV_DIR").unwrap());
    let base = if base.is_absolute() {
        base
    } else {
        std::env::current_dir().unwrap().join(base)
    };
    let expected = StoragePaths::dev(&base, std::env::consts::OS);
    let paths = StoragePaths::resolve().unwrap();
    assert_eq!(paths.config_path(), expected.config_path());
    assert_eq!(paths.layouts_dir(), expected.layouts_dir());
    paths.save_config("isolated").unwrap();
    assert_eq!(paths.load_config().unwrap(), "isolated");
}
