//! Application categories from desktop entries: the active window counts
//! as a game when its application's `.desktop` file lists the `Game`
//! category. Launchers tagged as games are skipped
//! ([`super::process::GAME_LAUNCHERS`]).

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use crate::runtime_state::ActiveWindow;

struct Index {
    stamp: Stamp,
    /// When `stamp` was last compared with the disk.
    checked: Instant,
    games: Catalog,
}

/// How often the desktop files are checked for changes: there may be
/// thousands, and detection runs every second while a game is on.
const RECHECK: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Catalog {
    ids: HashMap<String, bool>,
    aliases: HashMap<String, bool>,
}

impl Catalog {
    fn get(&self, name: &str) -> Option<&bool> {
        self.ids.get(name).or_else(|| self.aliases.get(name))
    }

    fn game(&self, window: &ActiveWindow) -> Option<String> {
        let app_id = window.app_id.trim().to_lowercase();
        // An explicit desktop id wins, even when it says "not a game".
        if let Some(game) = self.ids.get(&app_id) {
            return game.then_some(app_id);
        }
        let process = window
            .process_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        [app_id, process]
            .into_iter()
            .find(|key| !key.is_empty() && self.get(key) == Some(&true))
    }
}

type Fingerprint = (SystemTime, u64, i64, i64);
/// Directories and files: in-place writes do not touch the parent directory.
type Stamp = Vec<(PathBuf, Option<Fingerprint>)>;

static INDEX: Mutex<Option<Index>> = Mutex::new(None);

/// The name of the active window's application when it is a game.
pub(super) fn game_window(window: &ActiveWindow) -> Option<String> {
    if super::process::launcher_window(window) {
        return None;
    }
    let mut index = INDEX.lock().ok()?;
    let now = Instant::now();
    let stale = match index.as_mut() {
        None => true,
        Some(index) if now.duration_since(index.checked) < RECHECK => false,
        Some(index) => {
            index.checked = now;
            !current(&index.stamp)
        }
    };
    if stale {
        let (games, stamp) = build(&application_dirs());
        *index = Some(Index {
            games,
            stamp,
            checked: now,
        });
    }
    index.as_ref()?.games.game(window)
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

fn modified(path: &Path) -> Option<Fingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    Some((
        meta.modified().ok()?,
        meta.len(),
        meta.ctime(),
        meta.ctime_nsec(),
    ))
}

/// Whether no directory or file of `stamp` changed since.
fn current(stamp: &[(PathBuf, Option<Fingerprint>)]) -> bool {
    stamp.iter().all(|(dir, time)| modified(dir) == *time)
}

/// Index the entries of `dirs`; an id found earlier hides later ones, as
/// XDG lookup does. Also returns the directories read, with their times.
fn build(dirs: &[PathBuf]) -> (Catalog, Stamp) {
    let mut games = Catalog::default();
    let mut stamp = Vec::new();
    for dir in dirs {
        let mut entries = Vec::new();
        collect(dir, "", &mut entries, &mut stamp);
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (id, path) in entries {
            stamp.push((path.clone(), modified(&path)));
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let entry = parse(&text);
            if games.ids.contains_key(&id) {
                continue;
            }
            games.ids.insert(id, entry.game);
            for key in [entry.wm_class, entry.program].into_iter().flatten() {
                games
                    .aliases
                    .entry(key)
                    .and_modify(|game| *game &= entry.game)
                    .or_insert(entry.game);
            }
        }
    }
    (games, stamp)
}

/// Desktop files under `dir` with their ids: subdirectories join the id
/// with `-`. Each directory, even a missing one, goes to `stamp` before
/// it is read.
fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>, stamp: &mut Stamp) {
    stamp.push((dir.to_owned(), modified(dir)));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect(&path, &format!("{prefix}{name}-"), out, stamp);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            out.push((format!("{prefix}{stem}").to_lowercase(), path));
        }
    }
}

/// Programs that only start another one: their name says nothing about
/// the application.
const WRAPPERS: [&str; 20] = [
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
    "python",
    "python3",
    "node",
    "ruby",
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
    fn in_place_edits_and_alias_collisions_do_not_stale_or_override_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.desktop");
        std::fs::write(
            &file,
            "[Desktop Entry]\nCategories=Game;\nStartupWMClass=editor\nExec=shared\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("editor.desktop"),
            "[Desktop Entry]\nCategories=Utility;\nExec=shared\n",
        )
        .unwrap();
        let (games, stamp) = build(&[dir.path().to_owned()]);
        assert_eq!(games.get("editor"), Some(&false));
        assert_eq!(games.get("shared"), Some(&false));
        assert_eq!(
            games.game(&ActiveWindow {
                app_id: "editor".into(),
                process_name: Some("a".into()),
                ..ActiveWindow::default()
            }),
            None
        );
        let directory_time = std::fs::metadata(dir.path()).unwrap().modified().unwrap();
        std::fs::write(&file, "[Desktop Entry]\nCategories=Utility;\n").unwrap();
        assert_eq!(
            directory_time,
            std::fs::metadata(dir.path()).unwrap().modified().unwrap()
        );
        assert!(!current(&stamp));
        let (games, _) = build(&[dir.path().to_owned()]);
        assert_eq!(games.get("a"), Some(&false));
    }

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
        let (games, stamp) = build(&[user.path().into(), system.path().into()]);
        assert_eq!(games.get("org.example.puzzle"), Some(&true));
        assert_eq!(games.get("editor"), Some(&false));
        assert_eq!(games.get("vendor-shooter"), Some(&true));
        assert_eq!(games.get("shooterwindow"), Some(&true));
        assert_eq!(games.get("hidden"), Some(&false));
        assert_eq!(games.get("fgfs"), Some(&true));
        assert_eq!(games.get("env"), None);
        // A new entry in a subdirectory is noticed.
        assert!(current(&stamp));
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(
            system.path(),
            "vendor/racer.desktop",
            "[Desktop Entry]\nCategories=Game\n",
        );
        assert!(!current(&stamp));
    }
}
