//! Application categories from desktop entries: the active window counts
//! as a game when its application's `.desktop` file lists the `Game`
//! category. Launchers tagged as games are skipped
//! ([`super::process::GAME_LAUNCHERS`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::runtime_state::ActiveWindow;

/// Applications by lowercase desktop id, `StartupWMClass` and program
/// name: whether each is a game.
#[derive(Default)]
struct Index {
    /// Modification times of the `applications` directories it was built
    /// from; a change rebuilds it.
    stamp: Vec<Option<SystemTime>>,
    games: HashMap<String, bool>,
}

static INDEX: Mutex<Option<Index>> = Mutex::new(None);

/// The name of the active window's application when it is a game.
pub(super) fn game_window(window: &ActiveWindow) -> Option<String> {
    if super::process::launcher_window(window) {
        return None;
    }
    let dirs = application_dirs();
    let stamp = stamp(&dirs);
    let mut index = INDEX.lock().ok()?;
    if index.as_ref().is_none_or(|index| index.stamp != stamp) {
        *index = Some(Index {
            games: build(&dirs),
            stamp,
        });
    }
    let games = &index.as_ref()?.games;
    let app_id = window.app_id.trim().to_lowercase();
    let process = window.process_name.as_deref().map(str::to_lowercase);
    [Some(app_id.as_str()), process.as_deref()]
        .into_iter()
        .flatten()
        .filter(|key| !key.is_empty())
        .find(|key| games.get(*key) == Some(&true))
        .map(str::to_owned)
}

/// `applications` directories in XDG order: the user's first.
fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::data_dir);
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(std::env::split_paths(&system))
        .map(|dir| dir.join("applications"))
        .collect()
}

fn stamp(dirs: &[PathBuf]) -> Vec<Option<SystemTime>> {
    dirs.iter()
        .map(|dir| std::fs::metadata(dir).and_then(|meta| meta.modified()).ok())
        .collect()
}

/// Index the entries of `dirs`; an id found earlier hides later ones, as
/// XDG lookup does.
fn build(dirs: &[PathBuf]) -> HashMap<String, bool> {
    let mut games = HashMap::new();
    for dir in dirs {
        let mut entries = Vec::new();
        collect(dir, "", &mut entries);
        for (id, path) in entries {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let entry = parse(&text);
            if games.contains_key(&id) {
                continue;
            }
            games.insert(id, entry.game);
            for key in [entry.wm_class, entry.program].into_iter().flatten() {
                games.entry(key).or_insert(entry.game);
            }
        }
    }
    games
}

/// Desktop files under `dir` with their ids: subdirectories join the id
/// with `-`.
fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect(&path, &format!("{prefix}{name}-"), out);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            out.push((format!("{prefix}{stem}").to_lowercase(), path));
        }
    }
}

/// Programs that only start another one: their name says nothing about
/// the application.
const WRAPPERS: [&str; 16] = [
    "env",
    "sh",
    "bash",
    "flatpak",
    "snap",
    "wine",
    "wine64",
    "xdg-open",
    "gio",
    "gtk-launch",
    "kioclient",
    "kioclient5",
    "gamemoderun",
    "prime-run",
    "mangohud",
    "dbus-send",
];

#[derive(Default)]
struct Entry {
    game: bool,
    wm_class: Option<String>,
    /// Lowercase file name of the program `Exec` runs.
    program: Option<String>,
}

/// The program of an `Exec` line, unless it is a wrapper.
fn program(exec: &str) -> Option<String> {
    let first = exec.split_whitespace().next()?.trim_matches(['"', '\'']);
    let name = first.rsplit('/').next()?.to_lowercase();
    (!name.is_empty() && !WRAPPERS.contains(&name.as_str())).then_some(name)
}

/// The `[Desktop Entry]` keys that matter. A hidden entry is no game: it
/// only hides entries of the same id further down the search path.
fn parse(text: &str) -> Entry {
    let mut main = false;
    let mut entry = Entry::default();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            main = line == "[Desktop Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| main) else {
            continue;
        };
        match key.trim() {
            "Categories" => entry.game = value.split(';').any(|category| category.trim() == "Game"),
            "StartupWMClass" => {
                entry.wm_class = Some(value.trim().to_lowercase()).filter(|class| !class.is_empty())
            }
            "Exec" => entry.program = program(value),
            "Hidden" if value.trim() == "true" => return Entry::default(),
            _ => {}
        }
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_are_found_by_id_and_window_class() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        let write = |dir: &Path, name: &str, text: &str| {
            std::fs::create_dir_all(dir.join(name).parent().unwrap()).unwrap();
            std::fs::write(dir.join(name), text).unwrap();
        };
        write(
            system.path(),
            "org.example.Puzzle.desktop",
            "[Desktop Entry]\nName=Puzzle\nCategories=Game;LogicGame;\n",
        );
        write(
            system.path(),
            "editor.desktop",
            "[Desktop Entry]\nCategories=Utility;\nStartupWMClass=Editor\n[Desktop Action x]\nCategories=Game;\n",
        );
        write(
            system.path(),
            "org.example.Flight.desktop",
            "[Desktop Entry]\nExec=\"/opt/flight/fgfs\" --launcher\nCategories=Game;Simulation;\n",
        );
        write(
            system.path(),
            "shortcut.desktop",
            "[Desktop Entry]\nExec=env GAME=1 shortcut\nCategories=Game;\n",
        );
        write(
            system.path(),
            "vendor/shooter.desktop",
            "[Desktop Entry]\nCategories=Game\nStartupWMClass=ShooterWindow\n",
        );
        // The user's copy hides the system one.
        write(
            user.path(),
            "hidden.desktop",
            "[Desktop Entry]\nHidden=true\n",
        );
        write(
            system.path(),
            "hidden.desktop",
            "[Desktop Entry]\nCategories=Game;\n",
        );
        let games = build(&[user.path().into(), system.path().into()]);
        assert_eq!(games.get("org.example.puzzle"), Some(&true));
        assert_eq!(games.get("editor"), Some(&false));
        assert_eq!(games.get("vendor-shooter"), Some(&true));
        assert_eq!(games.get("shooterwindow"), Some(&true));
        assert_eq!(games.get("hidden"), Some(&false));
        assert_eq!(games.get("fgfs"), Some(&true));
        assert!(!games.contains_key("env"));
    }
}
