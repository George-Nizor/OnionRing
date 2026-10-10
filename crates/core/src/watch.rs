//! Picking up new games without a manual rescan.
//!
//! The app watches each source folder ([`watch_folders`]) and asks [`should_rescan`] about every
//! change it is told about. Only changes that can add, remove or change a game count; emulators
//! writing saves, logs and caches while you play do not. Changes arrive in bursts (copying one
//! ISO reports many), so wait for things to settle (a few seconds) before scanning.

use crate::catalog::Catalog;
use crate::db::Db;
use crate::scan::{ARCHIVE_EXTS, SKIP_DIRS};
use crate::util::{path_str, split_name};
use anyhow::Result;
use std::path::{Component, Path, PathBuf};

/// Folders emulators write to while a game runs (saves, trophies, NAND titles, temp files),
/// on top of the folders a scan skips anyway.
const BUSY_DIRS: &[&str] = &[
    "home", "savedata", "save", "saves", "trophy", "tmp", "temp", "shader", "log", "title", "shared2", "ticket",
    "meta", "import", "statesaves", "card a", "card b", "wfs", "crash_report",
];

/// Downloads and copies still in progress; the finished file's own change follows.
const UNFINISHED: &[&str] = &[".part", ".partial", ".crdownload", ".download", ".tmp", ".!qb", ".opdownload"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    File,
    Folder,
    /// Not known, usually because it was just removed.
    Unknown,
}

impl PathKind {
    /// What is at `path` now (links are followed); something that no longer exists is `Unknown`.
    pub fn probe(path: &Path) -> PathKind {
        match std::fs::metadata(path) {
            Ok(m) if m.is_dir() => PathKind::Folder,
            Ok(_) => PathKind::File,
            Err(_) => PathKind::Unknown,
        }
    }
}

/// The folders to watch: the library's sources that are reachable right now.
pub fn watch_folders(db: &Db) -> Result<Vec<PathBuf>> {
    Ok(db.sources()?.into_iter().filter(|s| s.exists).map(|s| PathBuf::from(s.path)).collect())
}

/// Whether a change at `path` (created, removed, renamed or written) should trigger a rescan.
/// Pure: nothing is read from disk, so it also works for paths that were just removed.
///
/// Yes for: a game file (a known extension), an archive, an emulator's program, a PS3/PS4/PS5
/// game folder marker (`PARAM.SFO`, `EBOOT.BIN`, `sce_sys\param.json`), and any folder
/// appearing or disappearing. No for anything outside `sources`, and for anything inside
/// folders emulators keep to themselves (saves, caches, BIOS, logs).
pub fn should_rescan(cat: &Catalog, sources: &[PathBuf], path: &Path, kind: PathKind) -> bool {
    let Some(rel) = relative_to_source(sources, path) else { return false };
    let names: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    let Some((name, parents)) = names.split_last() else { return false }; // the source itself
    if parents.iter().any(|p| ignored_dir(p))
        || name.starts_with('.')
        || name.starts_with("~$")
        || UNFINISHED.iter().any(|e| name.ends_with(e))
    {
        return false;
    }
    let (_, ext) = split_name(Path::new(name));
    let parent = parents.last().map(String::as_str).unwrap_or_default();
    let game_file = cat.is_rom_ext(&ext)
        || ARCHIVE_EXTS.contains(&ext.as_str())
        || (ext == ".exe" && cat.match_emulator_exe(name).is_some())
        || name == "param.sfo"
        || name == "eboot.bin"
        || (name == "param.json" && parent == "sce_sys");
    match kind {
        PathKind::File => game_file,
        PathKind::Folder => !ignored_dir(name),
        // Removed: a game file, or something without a file extension (a folder).
        PathKind::Unknown => game_file || (!looks_like_file(&ext) && !ignored_dir(name)),
    }
}

fn ignored_dir(name: &str) -> bool {
    name.starts_with('.') || name.ends_with("_old") || SKIP_DIRS.contains(&name) || BUSY_DIRS.contains(&name)
}

/// ".txt", ".ini", ".sav": an extension of plain letters and digits with a letter in it.
/// Folder names like "Okami (USA)", "Mr. Driller" or "Game v1.2" have none.
fn looks_like_file(ext: &str) -> bool {
    let e = ext.trim_start_matches('.');
    (1..=10).contains(&e.len()) && e.chars().all(|c| c.is_ascii_alphanumeric()) && e.chars().any(|c| c.is_ascii_alphabetic())
}

/// `path` relative to the deepest source holding it.
fn relative_to_source(sources: &[PathBuf], path: &Path) -> Option<PathBuf> {
    let norm = |p: &Path| {
        let s = path_str(p).replace('/', "\\");
        s.trim_start_matches(r"\\?\").trim_end_matches('\\').to_lowercase()
    };
    let p = norm(path);
    sources
        .iter()
        .filter_map(|s| {
            let s = norm(s);
            let rest = p.strip_prefix(s.as_str())?;
            (rest.is_empty() || rest.starts_with('\\')).then(|| (s.len(), rest.trim_start_matches('\\').to_string()))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, rest)| PathBuf::from(rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use PathKind::*;

    fn check(path: &str, kind: PathKind) -> bool {
        let cat = Catalog::load(None).unwrap();
        let sources = [PathBuf::from(r"G:\Emulation"), PathBuf::from(r"D:\Backups\Games\")];
        should_rescan(&cat, &sources, Path::new(path), kind)
    }

    #[test]
    fn games_trigger_a_rescan() {
        assert!(check(r"G:\Emulation\Playstation\PS2\Okami (USA).iso", File));
        assert!(check(r"g:\emulation\playstation\ps2\OKAMI.ISO", File));
        assert!(check(r"G:\Emulation\GC\Wind Waker.nkit.iso", File));
        assert!(check(r"G:\Emulation\Downloads\Ico (Europe).7z", File));
        assert!(check(r"G:\Emulation\PS3\rpcs3\dev_hdd0\game\BLUS30001\PARAM.SFO", File));
        assert!(check(r"G:\Emulation\PS4\CUSA00001\sce_sys\param.json", File));
        assert!(check(r"G:\Emulation\PS3\Demon's Souls\PS3_GAME\USRDIR\EBOOT.BIN", File));
        assert!(check(r"G:\Emulation\PS2\pcsx2-v2.4.0\pcsx2-qt.exe", File));
        // Folders appearing or disappearing.
        assert!(check(r"G:\Emulation\PS3\Demon's Souls", Folder));
        assert!(check(r"G:\Emulation\PS3\Demon's Souls", Unknown));
        assert!(check(r"G:\Emulation\PS2\Game v1.2", Unknown));
        assert!(check(r"G:\Emulation\PS2\Okami.iso", Unknown));
        // A source whose own path contains a skipped name still works below it.
        assert!(check(r"D:\Backups\Games\Okami.iso", File));
    }

    #[test]
    fn play_and_housekeeping_do_not() {
        assert!(!check(r"G:\Emulation", Folder));
        assert!(!check(r"C:\Users\me\Downloads\Okami.iso", File));
        assert!(!check(r"G:\EmulationOther\Okami.iso", File));
        assert!(!check(r"G:\Emulation\PS2\pcsx2\memcards\Mcd001.ps2", File));
        assert!(!check(r"G:\Emulation\PS2\pcsx2\inis\PCSX2.ini", File));
        assert!(!check(r"G:\Emulation\PS2\pcsx2\sstates", Folder));
        assert!(!check(r"G:\Emulation\PS2\pcsx2\cache\gamelist.cache", Unknown));
        assert!(!check(r"G:\Emulation\PS3\rpcs3\dev_hdd0\home\00000001\savedata\BLUS30001\PARAM.SFO", File));
        assert!(!check(r"G:\Emulation\PS3\rpcs3\dev_hdd0\home\00000001\savedata\BLUS30001", Folder));
        assert!(!check(r"G:\Emulation\PS3\rpcs3\RPCS3.log", File));
        assert!(!check(r"G:\Emulation\Wii\Dolphin\User\Wii\title\00010000\52534245\data", Folder));
        assert!(!check(r"G:\Emulation\Notes\param.json", File));
        assert!(!check(r"G:\Emulation\PS2\notes.txt", Unknown));
        assert!(!check(r"G:\Emulation\PS2\Okami.iso.part", File));
        assert!(!check(r"G:\Emulation\PS2\Okami.iso.crdownload", Unknown));
        assert!(!check(r"G:\Emulation\PS2\.staging", Folder));
        assert!(!check(r"G:\Emulation\Tools\helper.exe", File));
        assert!(!check(r"G:\Emulation\PS2\~$notes.docx", File));
    }

    #[test]
    fn probes_the_disk() {
        let t = crate::util::TempDir::new("watch");
        let f = t.file("a/b.iso", 1);
        assert_eq!(PathKind::probe(&f), File);
        assert_eq!(PathKind::probe(f.parent().unwrap()), Folder);
        assert_eq!(PathKind::probe(&t.path().join("gone")), Unknown);
    }
}
