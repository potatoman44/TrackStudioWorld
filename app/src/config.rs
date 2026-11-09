//! Paths the editor works with: saved in `~/.config/trackstudioworld/config.json`
//! (`%APPDATA%\trackstudioworld\config.json` on Windows) and overridable on the command line.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
usage: trackstudioworld [options]

  --romfs DIR       the game's extracted romfs (with Banc/, RSDB/, System/)
  --mods DIR        where mods are saved (default ~/MKW-Mods)
  --ns2jb DIR       Switch 2 SD card (or a folder to copy onto it): export there on every save
  --open FILE       open a course at start, e.g. \"Banc/Race_FirstCircuit_A_00[Game03].bcett.byml.zs\"
  --select NAME     after opening, select the first object of that type, or PATH#N for point N
                    of the first path with that name
  -h, --help        show this help
  -V, --version     show the version

Paths given here are also used for this session only; set them for good in File > Settings.";

/// Qt command-line options that take a value (QGuiApplication handles them).
const QT_OPTIONS_WITH_VALUE: [&str; 11] = [
    "-platform",
    "-platformpluginpath",
    "-platformtheme",
    "-plugin",
    "-qmljsdebugger",
    "-style",
    "-stylesheet",
    "-display",
    "-geometry",
    "-qwindowgeometry",
    "-qwindowtitle",
];

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub romfs: PathBuf,
    pub mods: PathBuf,
    /// Switch 2 SD card (or a folder copied to it) for ns2jb-modloader exports
    pub ns2jb: PathBuf,
    /// also export to `ns2jb` on every save
    pub ns2jb_on_save: bool,
    pub open: Option<String>,
    pub select: Option<String>,
}

fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Text shown in the UI hides the home folder (and so the user name): "/home/<user>/x" -> "~/x".
pub fn tilde(s: &str) -> String {
    let h = home();
    let h = h.to_string_lossy();
    let h = h.trim_end_matches(['/', '\\']);
    if h.is_empty() { s.to_string() } else { s.replace(h, "~") }
}

/// A path for a shell command shown to the user: "$HOME/x" when it is under the home folder.
pub fn shell_path(p: &Path) -> String {
    match p.strip_prefix(home()) {
        Ok(rest) => format!("$HOME/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

/// A path typed into a settings field or given on the command line: `~` and `~/...` expand to
/// the home folder, relative paths are taken from the current folder, and empty stays empty.
/// A file URL's path, or `s` as it is. On Windows the path of `file:///C:/x` is `/C:/x`, which
/// is not absolute there (a folder picker hands it over like that): the drive letter goes first.
fn url_path(s: &str, windows: bool) -> &str {
    let s = s.strip_prefix("file://").unwrap_or(s);
    let b = s.as_bytes();
    if windows && b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' { &s[1..] } else { s }
}

pub fn expand(s: &str) -> PathBuf {
    let s = s.trim();
    if s.is_empty() {
        return PathBuf::new();
    }
    let s = url_path(s, cfg!(windows));
    let p = match s.strip_prefix('~') {
        Some("") => home(),
        Some(rest) if rest.starts_with(['/', std::path::MAIN_SEPARATOR]) => {
            home().join(rest.trim_start_matches(['/', std::path::MAIN_SEPARATOR]))
        }
        _ => PathBuf::from(s),
    };
    if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) }
}

/// A per-user folder from an environment variable, or `fallback` under the home folder.
fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).filter(|v| !v.is_empty()).map_or_else(|| home().join(fallback), PathBuf::from)
}

/// Settings and other app data: `~/.config` (XDG), or the roaming `%APPDATA%` on Windows.
fn config_home() -> PathBuf {
    if cfg!(windows) { env_dir("APPDATA", "AppData/Roaming") } else { env_dir("XDG_CONFIG_HOME", ".config") }
}

pub fn config_file() -> PathBuf {
    config_home().join("trackstudioworld").join("config.json")
}

/// `~/.cache/trackstudioworld` (XDG), or `%LOCALAPPDATA%\trackstudioworld` on Windows.
pub fn cache_dir() -> PathBuf {
    let base = if cfg!(windows) { env_dir("LOCALAPPDATA", "AppData/Local") } else { env_dir("XDG_CACHE_HOME", ".cache") };
    base.join("trackstudioworld")
}

/// Eden's (and its forks') mod folder for this game: under `~/.local/share` (XDG), or
/// `%APPDATA%` on Windows.
pub fn emulator_load_dir() -> PathBuf {
    let base = if cfg!(windows) { config_home() } else { env_dir("XDG_DATA_HOME", ".local/share") };
    base.join("eden").join("load").join(mkwtrack::banc::TITLE_ID)
}

/// A `file:` URL for a local path, as QML image and texture sources take it.
pub fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    // `canonicalize` on Windows gives verbatim paths (`\\?\C:\...`)
    let s = s.strip_prefix("//?/").unwrap_or(&s);
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// [`file_url`] with the file's modification time as a query, so a file rewritten in place
/// (a re-imported or rebuilt model's textures and thumbnail) is not shown from Qt's cache.
pub fn versioned_url(p: &Path) -> String {
    let t = std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis());
    format!("{}?v={t}", file_url(p))
}

impl Config {
    /// The saved settings, then the command line on top. Exits on `--help`, `--version` or a
    /// bad command line.
    pub fn load() -> Config {
        let mut c = Self::saved();
        if let Err(e) = c.apply_args(std::env::args().skip(1)) {
            eprintln!("trackstudioworld: {e}\n\n{USAGE}");
            std::process::exit(2);
        }
        c
    }

    fn saved() -> Config {
        let mut c = Config::default();
        let v: Option<Value> = std::fs::read_to_string(config_file()).ok().and_then(|t| serde_json::from_str(&t).ok());
        if let Some(v) = v {
            let p = |k: &str| v.get(k).and_then(|x| x.as_str()).map(PathBuf::from).unwrap_or_default();
            c.romfs = p("romfs");
            c.mods = p("mods");
            c.ns2jb = p("ns2jb");
            c.ns2jb_on_save = v.get("ns2jbOnSave").and_then(|x| x.as_bool()).unwrap_or(false);
        }
        if c.mods.as_os_str().is_empty() {
            c.mods = home().join("MKW-Mods");
        }
        c
    }

    fn apply_args(&mut self, args: impl IntoIterator<Item = String>) -> Result<(), String> {
        let mut args = args.into_iter();
        while let Some(a) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{a} needs a value"));
            match a.as_str() {
                "--romfs" => self.romfs = expand(&value()?),
                "--mods" => self.mods = expand(&value()?),
                // models used to come from a separate FBX export; they are read from the romfs now
                "--models" => {
                    value()?;
                    eprintln!("trackstudioworld: --models is no longer needed (models are read from the romfs); ignored");
                }
                "--ns2jb" => {
                    self.ns2jb = expand(&value()?);
                    self.ns2jb_on_save = true;
                }
                "--open" => self.open = Some(value()?),
                "--select" => self.select = Some(value()?),
                "-h" | "--help" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    println!("trackstudioworld {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                // Qt's own options (-platform xcb, -reverse...) are read by QGuiApplication
                s if s.starts_with('-') && !s.starts_with("--") => {
                    if QT_OPTIONS_WITH_VALUE.contains(&s) {
                        args.next();
                    }
                }
                s => return Err(format!("unknown option {s}")),
            }
        }
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let f = config_file();
        let v = json!({"romfs": self.romfs, "mods": self.mods, "ns2jb": self.ns2jb, "ns2jbOnSave": self.ns2jb_on_save});
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        mkwtrack::io::write_atomic(&f, text.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn valid(&self) -> Result<(), String> {
        if !self.romfs.join("Banc").is_dir() {
            return Err(format!("{} has no Banc folder: set the romfs folder in Settings", self.romfs.display()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls() {
        assert_eq!(file_url(Path::new("/a/b c.png")), "file:///a/b c.png");
        assert_eq!(file_url(Path::new("C:\\Users\\x\\t.png")), "file:///C:/Users/x/t.png");
        assert_eq!(file_url(Path::new("\\\\?\\C:\\x")), "file:///C:/x");
    }

    #[test]
    fn expands_only_a_leading_tilde() {
        let h = home();
        assert_eq!(expand("~"), h);
        assert_eq!(expand("~/x/y"), h.join("x/y"));
        assert!(expand("a~b").ends_with("a~b"));
        assert_eq!(expand("  "), PathBuf::new());
        assert_eq!(expand("/abs"), PathBuf::from("/abs"));
    }

    #[test]
    fn file_urls_and_windows_url_paths() {
        assert_eq!(expand("file:///home/x/romfs"), PathBuf::from("/home/x/romfs"));
        // what a folder picker hands over on Windows: the drive letter must not get a slash
        assert_eq!(url_path("/C:/Users/x/romfs", true), "C:/Users/x/romfs");
        assert_eq!(url_path("file:///C:/Users/x/romfs", true), "C:/Users/x/romfs");
        assert_eq!(url_path("/C:/x", false), "/C:/x");
        assert_eq!(url_path("/home/x", true), "/home/x");
    }

    #[test]
    fn arguments() {
        let mut c = Config::default();
        let args = ["--romfs", "/r", "-platform", "xcb", "--open", "Banc/X.bcett.byml.zs", "--ns2jb", "/sd", "-reverse"].map(String::from);
        c.apply_args(args).unwrap();
        assert_eq!((c.romfs.as_path(), c.open.as_deref(), c.ns2jb_on_save), (Path::new("/r"), Some("Banc/X.bcett.byml.zs"), true));
        assert!(c.apply_args(["--bogus".to_string()]).is_err());
        assert!(c.apply_args(["--mods".to_string()]).is_err());
    }
}
