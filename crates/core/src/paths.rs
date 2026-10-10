//! Where things live.
//!
//! The app itself stays small (installed per-user by the installer). Everything it gathers:
//! the library database, artwork, installed ports/emulators and downloads, lives under a
//! user-chosen *data root* such as `G:\OnionRing`. A tiny pointer file in `%APPDATA%\OnionRing`
//! remembers which data root to use, so reinstalling the app and pointing it at the same
//! folder restores everything.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub data_root: Option<PathBuf>,
}

impl AppConfig {
    pub fn file() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("OnionRing").join("config.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let f = Self::file();
        std::fs::create_dir_all(f.parent().unwrap())?;
        std::fs::write(&f, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DataRoot {
    pub root: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    app: String,
    format: u32,
    created: i64,
}

impl DataRoot {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let dr = DataRoot { root: root.into() };
        dr.ensure()?;
        Ok(dr)
    }

    fn ensure(&self) -> Result<()> {
        for d in [self.media(), self.apps(), self.downloads(), self.cache()] {
            std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
        }
        let m = self.root.join("onionring.json");
        if !m.exists() {
            let manifest = Manifest { app: "OnionRing".into(), format: 1, created: crate::util::now() };
            std::fs::write(&m, serde_json::to_string_pretty(&manifest)?)?;
        }
        Ok(())
    }

    pub fn db(&self) -> PathBuf { self.root.join("library.db") }
    pub fn media(&self) -> PathBuf { self.root.join("Media") }
    pub fn apps(&self) -> PathBuf { self.root.join("Apps") }
    pub fn downloads(&self) -> PathBuf { self.root.join("Downloads") }
    pub fn cache(&self) -> PathBuf { self.root.join("Cache") }
    pub fn reclaim(&self) -> PathBuf { self.root.join("Reclaim") }
    /// Save backups, one folder per run (created on the first backup).
    pub fn backups(&self) -> PathBuf { self.root.join("Backups") }
    pub fn local_catalog(&self) -> PathBuf { self.root.join("catalog.local.json") }
    pub fn media_dir(&self, game_key: &str) -> PathBuf { self.media().join(crate::util::short_hash(game_key)) }
}

fn drives() -> Vec<PathBuf> {
    (b'C'..=b'Z')
        .map(|c| PathBuf::from(format!("{}:\\", c as char)))
        .filter(|p| p.exists())
        .collect()
}

/// A sensible default for the data root: an existing OnionRing folder, otherwise the drive that
/// already holds an emulation setup, otherwise the first non-system drive, otherwise the profile.
pub fn suggest_data_root() -> PathBuf {
    let ds = drives();
    if let Some(d) = ds.iter().find(|d| d.join("OnionRing").join("onionring.json").exists()) {
        return d.join("OnionRing");
    }
    if let Some(e) = suggest_emulation_dirs().first()
        && let Some(anc) = e.ancestors().last() {
            return anc.join("OnionRing");
        }
    if let Some(d) = ds.iter().find(|d| !d.starts_with("C:\\")) {
        return d.join("OnionRing");
    }
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\")).join("OnionRing")
}

/// Folders that look like an existing emulation setup, to offer during first run.
pub fn suggest_emulation_dirs() -> Vec<PathBuf> {
    let names = ["Emulation", "Emulators", "Emulator", "ROMs", "Roms", "Retro", "EmuDeck\\Emulation"];
    let mut bases = drives();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        bases.push(home.join("Documents"));
        bases.push(home);
    }
    let mut out = Vec::new();
    for b in &bases {
        for n in names {
            let p = b.join(n);
            if p.is_dir() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

pub fn same_volume(a: &Path, b: &Path) -> bool {
    let root = |p: &Path| p.components().next().map(|c| c.as_os_str().to_string_lossy().to_uppercase());
    root(a).is_some() && root(a) == root(b)
}

/// Expand a path template (used by emulator checks and save backups) into every path it can
/// mean on this PC. Nothing is checked on disk.
///
/// - `{emu}`: the emulator's folder. Without one, the template expands to nothing.
/// - `%APPDATA%`, `%LOCALAPPDATA%`, `%USERPROFILE%`: the usual Windows folders.
/// - `%DOCUMENTS%`: the Documents folder. Windows can move it into OneDrive, so both the local
///   and the OneDrive one are returned.
///
/// `/` and `\` both work as separators. Any other `%VAR%` expands to nothing.
pub fn expand(template: &str, emu_dir: Option<&Path>) -> Vec<PathBuf> {
    expand_with(template, emu_dir, &|k| std::env::var(k).ok())
}

/// [`expand`] with environment lookups supplied by the caller (for tests).
pub fn expand_with(template: &str, emu_dir: Option<&Path>, env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    static VAR: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\{emu\}|%([A-Za-z_]+)%").unwrap());
    let get = |k: &str| env(k).filter(|v| !v.trim().is_empty());
    let mut outs = vec![String::new()];
    let mut last = 0;
    for m in VAR.captures_iter(template) {
        let whole = m.get(0).unwrap();
        let values: Vec<String> = match m.get(1).map(|n| n.as_str().to_ascii_uppercase()) {
            None => emu_dir.map(|d| d.to_string_lossy().to_string()).into_iter().collect(),
            Some(n) if n == "DOCUMENTS" => ["USERPROFILE", "OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
                .iter()
                .filter_map(|k| get(k))
                .map(|base| format!("{base}\\Documents"))
                .collect(),
            Some(n) if ["APPDATA", "LOCALAPPDATA", "USERPROFILE"].contains(&n.as_str()) => get(&n).into_iter().collect(),
            Some(_) => vec![],
        };
        if values.is_empty() {
            return vec![];
        }
        let lit = &template[last..whole.start()];
        outs = outs.iter().flat_map(|o| values.iter().map(move |v| format!("{o}{lit}{v}"))).collect();
        last = whole.end();
    }
    let mut seen = std::collections::HashSet::new();
    outs.into_iter()
        .map(|o| tidy_separators(&format!("{o}{}", &template[last..])))
        .filter(|p| seen.insert(p.to_lowercase()))
        .map(PathBuf::from)
        .collect()
}

/// Backslashes only, no doubled separators (a leading `\\` for network paths is kept).
fn tidy_separators(p: &str) -> String {
    let p = p.replace('/', "\\");
    let (head, rest) = match p.strip_prefix("\\\\") {
        Some(rest) => ("\\\\", rest),
        None => ("", p.as_str()),
    };
    let mut out = String::from(head);
    for c in rest.chars() {
        if !(c == '\\' && out.ends_with('\\') && out.len() > head.len()) {
            out.push(c);
        }
    }
    out
}

/// Resolve `.` and `..` without touching the disk (`C:\a\inis\..\bios` -> `C:\a\bios`).
pub fn clean(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(k: &str) -> Option<String> {
        match k {
            "APPDATA" => Some(r"C:\Users\me\AppData\Roaming".into()),
            "USERPROFILE" => Some(r"C:\Users\me".into()),
            "OneDrive" => Some(r"C:\Users\me\OneDrive".into()),
            _ => None,
        }
    }

    #[test]
    fn expands_templates() {
        let emu = Path::new(r"G:\Emu\PCSX2");
        assert_eq!(expand_with("{emu}/bios/*.bin", Some(emu), &env), vec![PathBuf::from(r"G:\Emu\PCSX2\bios\*.bin")]);
        assert_eq!(expand_with("%APPDATA%/Cemu/keys.txt", None, &env), vec![PathBuf::from(r"C:\Users\me\AppData\Roaming\Cemu\keys.txt")]);
        assert_eq!(
            expand_with("%DOCUMENTS%/PCSX2/memcards", None, &env),
            vec![PathBuf::from(r"C:\Users\me\Documents\PCSX2\memcards"), PathBuf::from(r"C:\Users\me\OneDrive\Documents\PCSX2\memcards")]
        );
        // No emulator folder, an unset variable or an unknown one: nothing.
        assert!(expand_with("{emu}/bios", None, &env).is_empty());
        assert!(expand_with("%LOCALAPPDATA%/DuckStation", None, &env).is_empty());
        assert!(expand_with("%WINDIR%/System32", None, &env).is_empty());
        // A drive root does not leave a doubled separator.
        assert_eq!(expand_with("{emu}/keys.txt", Some(Path::new("D:\\")), &env), vec![PathBuf::from(r"D:\keys.txt")]);
    }

    #[test]
    fn cleans_dots() {
        assert_eq!(clean(Path::new(r"G:\PCSX2\inis\..\bios\.\PS2")), PathBuf::from(r"G:\PCSX2\bios\PS2"));
        assert_eq!(clean(Path::new(r"C:\..\x")), PathBuf::from(r"C:\x"));
    }
}
