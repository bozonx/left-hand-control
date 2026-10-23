//! Watcher integration in its own process: no global-state races with unit tests.

use lhc_core::{
    StoragePaths,
    config_document::ConfigDocument,
    gamemode::{self, GameModeControl},
};
use std::time::{Duration, Instant};

fn wait_for(check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "game-mode state did not converge: {:?}",
            gamemode::status()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Watcher;
impl Drop for Watcher {
    fn drop(&mut self) {
        gamemode::stop_watcher();
    }
}

#[test]
fn startup_and_watcher_restart_follow_persisted_control() {
    let directory = tempfile::tempdir().unwrap();
    let paths = StoragePaths::new(
        directory.path().join("settings"),
        directory.path().join("data"),
    );
    paths.save_config(r#"{"version":1,"settings":{"gameMode":{"detectors":{"gamemoded":false,"steam":false,"lutris":false,"heroic":false,"umu":false,"gamescope":false,"appCategory":false}}},"state":{"gameMode":"on"}}"#).unwrap();
    gamemode::start_watcher(Some(paths.clone()));
    let _watcher = Watcher;
    wait_for(|| gamemode::status().control == GameModeControl::On);
    assert!(gamemode::status().active);
    let mut document = ConfigDocument::load(paths.clone()).unwrap();
    let mut raw: serde_json::Value = serde_json::from_str(&paths.load_config().unwrap()).unwrap();
    raw["state"]["gameMode"] = "off".into();
    paths.save_config(&raw.to_string()).unwrap();
    assert_eq!(document.settings().game_mode.control, GameModeControl::On);
    document = ConfigDocument::load(paths.clone()).unwrap();
    assert_eq!(gamemode::status().control, GameModeControl::Off);
    assert!(!gamemode::status().active);
    document
        .update_settings(|settings| settings.game_mode.control = GameModeControl::Auto)
        .unwrap();
    wait_for(|| gamemode::status().state_available);
    assert!(!gamemode::status().detection_enabled);
    gamemode::stop_watcher();
    gamemode::start_watcher(Some(paths));
    document
        .update_settings(|settings| settings.game_mode.control = GameModeControl::On)
        .unwrap();
    assert!(gamemode::status().active);
    #[cfg(target_os = "linux")]
    check_grace_period(&mut document);
}

#[cfg(target_os = "linux")]
fn check_grace_period(document: &mut ConfigDocument) {
    use lhc_core::profile::model::GameModeProcessMatcher;
    use std::os::unix::process::CommandExt;
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let name = format!("lhc-game-test-{}", std::process::id());
    let mut child = Child(
        std::process::Command::new("sleep")
            .arg("30")
            .arg0(&name)
            .spawn()
            .unwrap(),
    );
    document
        .update_settings(|settings| {
            settings.game_mode.control = GameModeControl::Auto;
            settings.game_mode.process_matchers = vec![GameModeProcessMatcher {
                id: "test-game".into(),
                name,
                only_active_window: false,
                is_blacklist: false,
            }];
        })
        .unwrap();
    wait_for(|| gamemode::status().active);
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let stopped = Instant::now();
    // A re-evaluation on an unrelated save must preserve the recent match.
    document
        .set_appearance(lhc_core::profile::model::Appearance::Dark)
        .unwrap();
    assert!(gamemode::status().active);
    wait_for(|| !gamemode::status().active);
    assert!(stopped.elapsed() >= Duration::from_secs(2));
    assert!(gamemode::status().state_available);
}
