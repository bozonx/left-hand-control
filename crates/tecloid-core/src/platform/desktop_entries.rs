//! Application actions from freedesktop desktop entries (the `Actions=`
//! key, e.g. "New Private Window"), offered as `xdg:desktop-id/action`.
//! Every Linux desktop reads the same files from `$XDG_DATA_HOME` and
//! `$XDG_DATA_DIRS`, so this works on KDE, GNOME and the rest alike.
//! Spec: <https://specifications.freedesktop.org/desktop-entry-spec/latest/>.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One action of an installed application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppAction {
    /// Desktop-file id without `.desktop`, e.g. `org.mozilla.firefox`.
    pub app_id: String,
    /// Action id from `Actions=`, e.g. `new-private-window`.
    pub action: String,
    /// Localized application name.
    pub app_name: String,
    /// Localized action name.
    pub label: String,
}

impl AppAction {
    /// The `xdg:` id of this action, without the prefix.
    pub fn id(&self) -> String {
        format!("{}/{}", self.app_id, self.action)
    }
}

/// How to run a desktop-entry action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// `org.freedesktop.Application.ActivateAction` on a D-Bus activatable
    /// application.
    Activate {
        bus_name: String,
        path: String,
        action: String,
    },
    /// The action's `Exec` command line, split into program and arguments.
    Exec(Vec<String>),
}

/// Actions of every application visible on this desktop, sorted by
/// application and action name. Unreadable files are skipped.
pub fn list() -> Vec<AppAction> {
    let desktops = current_desktops();
    let languages = languages();
    let mut out = Vec::new();
    for (app_id, path) in desktop_files() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let entry = Entry::parse(&text);
        if !entry.visible(&desktops) {
            continue;
        }
        let app_name = entry.main.localized("Name", &languages).unwrap_or(&app_id);
        for action in entry.actions() {
            let Some(group) = entry.action_group(action) else {
                continue;
            };
            let Some(label) = group.localized("Name", &languages) else {
                continue;
            };
            if entry.launch(action).is_none() {
                continue;
            }
            out.push(AppAction {
                app_id: app_id.clone(),
                action: action.to_owned(),
                app_name: app_name.to_owned(),
                label: label.to_owned(),
            });
        }
    }
    out.sort_by(|a, b| {
        (a.app_name.to_lowercase(), a.label.to_lowercase())
            .cmp(&(b.app_name.to_lowercase(), b.label.to_lowercase()))
    });
    out
}

/// How to run `action` of application `app_id`, or `None` when the
/// application or the action is not installed.
pub fn resolve(app_id: &str, action: &str) -> Option<Launch> {
    let path = find(app_id)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let entry = Entry::parse(&text);
    if entry.main.get("Hidden") == Some("true") || !entry.actions().any(|known| known == action) {
        return None;
    }
    entry.launch(action).map(|launch| match launch {
        EntryLaunch::Activate => Launch::Activate {
            bus_name: app_id.to_owned(),
            path: format!("/{}", app_id.replace('.', "/").replace('-', "_")),
            action: action.to_owned(),
        },
        EntryLaunch::Exec(exec) => Launch::Exec(expand_exec(exec, &entry, &path.to_string_lossy())),
    })
}

/// `applications/` directories, most important first.
fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")));
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(
            system
                .split(':')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        )
        .map(|dir| dir.join("applications"))
        .collect()
}

/// Every desktop file by id; an id found in an earlier directory hides the
/// same id further down, as the spec requires.
fn desktop_files() -> Vec<(String, PathBuf)> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for dir in application_dirs() {
        let mut files = Vec::new();
        collect(&dir, &dir, &mut files);
        for (id, path) in files {
            if seen.insert(id.clone(), ()).is_none() {
                out.push((id, path));
            }
        }
    }
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if let Some(id) = desktop_id(root, &path) {
            out.push((id, path));
        }
    }
}

/// `kde4/foo.desktop` under `root` has the id `kde4-foo`.
fn desktop_id(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    let id = relative.strip_suffix(".desktop")?;
    Some(id.replace('/', "-"))
}

fn find(app_id: &str) -> Option<PathBuf> {
    application_dirs()
        .into_iter()
        .map(|dir| dir.join(format!("{app_id}.desktop")))
        .find(|path| path.is_file())
        .or_else(|| {
            desktop_files()
                .into_iter()
                .find(|(id, _)| id == app_id)
                .map(|(_, path)| path)
        })
}

/// Names this desktop matches in `OnlyShowIn` / `NotShowIn`.
fn current_desktops() -> Vec<&'static str> {
    use super::linux::{Desktop, detect};
    match detect().desktop {
        Desktop::Kde => vec!["KDE"],
        Desktop::Gnome => vec!["GNOME"],
        Desktop::Sway => vec!["sway"],
        Desktop::Hyprland => vec!["Hyprland"],
        Desktop::Xfce => vec!["XFCE"],
        Desktop::Cinnamon => vec!["X-Cinnamon", "Cinnamon"],
        Desktop::Mate => vec!["MATE"],
        Desktop::Lxqt => vec!["LXQt"],
        Desktop::Unity => vec!["Unity"],
        Desktop::Pantheon => vec!["Pantheon"],
        Desktop::Unknown => vec![],
    }
}

/// Locale keys to try, most specific first: `ru_RU@mod`, `ru_RU`, `ru@mod`, `ru`.
fn languages() -> Vec<String> {
    let mut out = Vec::new();
    for value in super::locale::message_languages() {
        let (base, modifier) = match value.split_once('@') {
            Some((base, modifier)) => (base, Some(modifier)),
            None => (value.as_str(), None),
        };
        let base = base.split('.').next().unwrap_or(base);
        let lang = base.split('_').next().unwrap_or(base);
        let mut push = |key: String| {
            if !out.contains(&key) {
                out.push(key);
            }
        };
        if let Some(modifier) = modifier {
            push(format!("{base}@{modifier}"));
        }
        push(base.to_owned());
        if let Some(modifier) = modifier {
            push(format!("{lang}@{modifier}"));
        }
        push(lang.to_owned());
    }
    out
}

/// One `[group]` of a desktop file: key → value, localized keys included
/// as `Name[ru]`.
#[derive(Default)]
struct Group<'a>(HashMap<&'a str, &'a str>);

impl<'a> Group<'a> {
    fn get(&self, key: &str) -> Option<&'a str> {
        self.0.get(key).copied()
    }

    fn localized(&self, key: &str, languages: &[String]) -> Option<&'a str> {
        languages
            .iter()
            .find_map(|lang| self.get(&format!("{key}[{lang}]")))
            .or_else(|| self.get(key))
            .filter(|value| !value.trim().is_empty())
    }

    fn list(&self, key: &str) -> impl Iterator<Item = &'a str> {
        self.get(key)
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|item| !item.is_empty())
    }
}

enum EntryLaunch<'a> {
    Activate,
    Exec(&'a str),
}

struct Entry<'a> {
    main: Group<'a>,
    groups: HashMap<&'a str, Group<'a>>,
}

impl<'a> Entry<'a> {
    fn parse(text: &'a str) -> Self {
        let mut groups: HashMap<&str, Group> = HashMap::new();
        let mut current: Option<&str> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                current = Some(name);
                groups.entry(name).or_default();
            } else if let (Some(group), Some((key, value))) = (current, line.split_once('=')) {
                groups
                    .entry(group)
                    .or_default()
                    .0
                    .entry(key.trim())
                    .or_insert(value.trim());
            }
        }
        let main = groups.remove("Desktop Entry").unwrap_or_default();
        Self { main, groups }
    }

    fn visible(&self, desktops: &[&str]) -> bool {
        let main = &self.main;
        if main.get("Type") != Some("Application")
            || main.get("Hidden") == Some("true")
            || main.get("NoDisplay") == Some("true")
        {
            return false;
        }
        let only: Vec<_> = main.list("OnlyShowIn").collect();
        if !only.is_empty() && !only.iter().any(|name| desktops.contains(name)) {
            return false;
        }
        if main.list("NotShowIn").any(|name| desktops.contains(&name)) {
            return false;
        }
        main.get("TryExec").is_none_or(executable_exists)
    }

    fn actions(&self) -> impl Iterator<Item = &'a str> {
        self.main.list("Actions")
    }

    fn action_group(&self, action: &str) -> Option<&Group<'a>> {
        self.groups.get(format!("Desktop Action {action}").as_str())
    }

    /// D-Bus activation wins when the application supports it; otherwise
    /// the action needs its own `Exec`.
    fn launch(&self, action: &str) -> Option<EntryLaunch<'a>> {
        if self.main.get("DBusActivatable") == Some("true") {
            return Some(EntryLaunch::Activate);
        }
        self.action_group(action)?
            .get("Exec")
            .filter(|exec| !exec.trim().is_empty())
            .map(EntryLaunch::Exec)
    }
}

fn executable_exists(program: &str) -> bool {
    let program = program.trim();
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Undo the string escapes of a desktop-file value (`\s`, `\n`, `\t`,
/// `\r`, `\\`).
fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Split an `Exec` value into arguments: blanks separate them, double
/// quotes group them, and inside quotes `\"`, `` \` ``, `\$` and `\\` are
/// escapes.
fn split_exec(exec: &str) -> Vec<(String, bool)> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut in_arg = false;
    let mut was_quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_arg = true;
                was_quoted = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_arg {
                    args.push((std::mem::take(&mut current), was_quoted));
                    in_arg = false;
                    was_quoted = false;
                }
            }
            c => {
                current.push(c);
                in_arg = true;
            }
        }
    }
    if in_arg {
        args.push((current, was_quoted));
    }
    args
}

/// The command line of an `Exec` value with its field codes expanded. No
/// files or URLs are passed, so `%f`, `%u` and the like disappear.
fn expand_exec(exec: &str, entry: &Entry, file: &str) -> Vec<String> {
    let name = entry.main.get("Name").unwrap_or_default();
    let icon = entry.main.get("Icon").unwrap_or_default();
    let mut out = Vec::new();
    for (arg, quoted) in split_exec(&unescape_value(exec)) {
        if !quoted {
            match arg.as_str() {
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => continue,
                "%i" => {
                    if !icon.is_empty() {
                        out.push("--icon".into());
                        out.push(icon.into());
                    }
                    continue;
                }
                _ => {}
            }
        }
        let mut expanded = String::with_capacity(arg.len());
        let mut chars = arg.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                expanded.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => expanded.push('%'),
                Some('c') => expanded.push_str(name),
                Some('k') => expanded.push_str(file),
                Some('i') if !icon.is_empty() => expanded.push_str(icon),
                _ => {}
            }
        }
        out.push(expanded);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]
Type=Application
Name=Firefox
Name[ru]=Фаерфокс
Icon=firefox
Exec=firefox %u
Actions=new-window;new-private-window;broken;

[Desktop Action new-window]
Name=New Window
Name[ru]=Новое окно
Exec=firefox --new-window %u

[Desktop Action new-private-window]
Name=New Private Window
Exec=\"/opt/fire fox/firefox\" --private-window --class=\"a \\\\\"b\\\\\"\" %%done %i

[Desktop Action broken]
Name=Broken
";

    #[test]
    fn reads_actions_and_their_localized_names() {
        let entry = Entry::parse(FIREFOX);
        assert!(entry.visible(&["KDE"]));
        let ru = vec!["ru_RU".to_string(), "ru".to_string()];
        assert_eq!(entry.main.localized("Name", &ru), Some("Фаерфокс"));
        assert_eq!(
            entry
                .action_group("new-window")
                .unwrap()
                .localized("Name", &ru),
            Some("Новое окно")
        );
        assert_eq!(
            entry.actions().collect::<Vec<_>>(),
            ["new-window", "new-private-window", "broken"]
        );
        assert!(entry.launch("broken").is_none());
    }

    #[test]
    fn exec_is_split_and_field_codes_expand() {
        let entry = Entry::parse(FIREFOX);
        let Some(EntryLaunch::Exec(exec)) = entry.launch("new-private-window") else {
            panic!("no exec");
        };
        assert_eq!(
            expand_exec(exec, &entry, "/x.desktop"),
            [
                "/opt/fire fox/firefox",
                "--private-window",
                "--class=a \"b\"",
                "%done",
                "--icon",
                "firefox"
            ]
        );
        let Some(EntryLaunch::Exec(exec)) = entry.launch("new-window") else {
            panic!("no exec");
        };
        assert_eq!(expand_exec(exec, &entry, ""), ["firefox", "--new-window"]);
    }

    #[test]
    fn dbus_activatable_apps_and_visibility() {
        let entry = Entry::parse(
            "[Desktop Entry]\nType=Application\nName=X\nDBusActivatable=true\nOnlyShowIn=GNOME;\nActions=a;\n[Desktop Action a]\nName=A\n",
        );
        assert!(matches!(entry.launch("a"), Some(EntryLaunch::Activate)));
        assert!(entry.visible(&["GNOME"]));
        assert!(!entry.visible(&["KDE"]));
        let hidden = Entry::parse("[Desktop Entry]\nType=Application\nNoDisplay=true\n");
        assert!(!hidden.visible(&["KDE"]));
    }

    #[test]
    fn desktop_ids_follow_subdirectories() {
        let root = Path::new("/usr/share/applications");
        assert_eq!(
            desktop_id(root, &root.join("kde4/foo.desktop")).as_deref(),
            Some("kde4-foo")
        );
        assert_eq!(desktop_id(root, &root.join("foo.txt")), None);
    }
}
