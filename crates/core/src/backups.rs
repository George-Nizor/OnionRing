//! Save backups.
//!
//! One run zips every save folder OnionRing knows about into
//! `<data root>\Backups\<YYYY-MM-DD_HHMM>\<name>.zip`, one archive per folder: the emulator save
//! folders listed in the catalog (`saves` in emulators.json, for emulators found on the PC or
//! installed by OnionRing) and the `persist` folders of installed ports. Archives are made with
//! Windows' own tar.exe. Nothing is ever deleted or overwritten; each run gets its own folder.

use crate::catalog::Catalog;
use crate::db::Db;
use crate::install;
use crate::model::Progress;
use crate::paths::{self, DataRoot};
use crate::util::{self, now, path_str};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Written into each backup folder: what was saved and from where.
const MANIFEST: &str = "backup.json";

/// A folder of saves that a backup covers.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveSource {
    /// Plain name, also the archive's name: "PCSX2", "Dolphin (Wii)", "Bloodborne".
    pub name: String,
    pub path: String,
    /// Names of files and folders left out (installed game data inside a save tree).
    pub skip: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    pub name: String,
    /// The folder it came from (for a "show files" view).
    pub from: String,
    /// Size of the archive in bytes.
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// The folder's name, e.g. "2026-10-10_1532".
    pub name: String,
    pub created: i64,
    /// All archives together, in bytes.
    pub size: u64,
    pub path: String,
    pub items: Vec<BackupItem>,
    /// Plain-language problems ("Could not save PCSX2: ..."). Everything else was saved.
    #[serde(default)]
    pub failed: Vec<String>,
}

/// Every save folder that exists and holds something, named for people.
pub fn save_sources(root: &DataRoot, db: &Db, cat: &Catalog) -> Result<Vec<SaveSource>> {
    sources_with(root, db, cat, &|k| std::env::var(k).ok())
}

fn sources_with(root: &DataRoot, db: &Db, cat: &Catalog, env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<SaveSource>> {
    let rows = db.emulator_rows()?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<SaveSource> = Vec::new();
    let take = |p: &Path, seen: &mut HashSet<String>| {
        let key = std::fs::canonicalize(p).map(|c| path_str(&c)).unwrap_or_else(|_| path_str(p)).to_lowercase();
        p.is_dir() && has_files(p) && seen.insert(key)
    };

    for def in cat.emulators.iter().filter(|d| !d.saves.is_empty()) {
        // Every copy of this emulator on the PC, OnionRing's own included.
        let dirs: Vec<PathBuf> = rows
            .iter()
            .filter(|r| cat.emulator_for_instance(&r.0).is_some_and(|d| d.id == def.id))
            .filter_map(|r| Path::new(&r.2).parent().map(Path::to_path_buf))
            .collect();
        let mut found: Vec<PathBuf> = Vec::new();
        for t in &def.saves {
            let expanded: Vec<PathBuf> = if t.contains("{emu}") {
                dirs.iter().flat_map(|d| paths::expand_with(t, Some(d), env)).collect()
            } else {
                paths::expand_with(t, None, env)
            };
            found.extend(expanded.into_iter().filter(|p| take(p, &mut seen)));
        }
        let labels: Vec<String> =
            found.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).collect();
        let distinct = labels.iter().collect::<HashSet<_>>().len() == labels.len();
        for (i, p) in found.iter().enumerate() {
            let name = match (found.len(), distinct) {
                (1, _) => def.name.clone(),
                (_, true) => format!("{} ({})", def.name, labels[i]),
                (_, false) if i == 0 => def.name.clone(),
                (_, false) => format!("{} {}", def.name, i + 1),
            };
            out.push(SaveSource { name, path: path_str(p), skip: def.saves_skip.clone() });
        }
    }

    // Ports keep their saves and settings in Apps\<id>\persist.
    let mut ports: Vec<_> = install::installed_apps(root).into_iter().filter(|a| a.kind == "port").collect();
    ports.sort_by(|a, b| a.id.cmp(&b.id));
    for app in ports {
        let p = root.apps().join(&app.id).join("persist");
        if take(&p, &mut seen) {
            let name = cat.port(&app.id).map(|d| d.game.clone()).unwrap_or(app.name);
            out.push(SaveSource { name, path: path_str(&p), skip: vec![] });
        }
    }

    // Archive names must not collide (two ports for the same game, say).
    let mut used: HashMap<String, usize> = HashMap::new();
    for s in out.iter_mut() {
        let n = used.entry(file_name(&s.name).to_lowercase()).or_insert(0);
        *n += 1;
        if *n > 1 {
            s.name = format!("{} {}", s.name, n);
        }
    }
    Ok(out)
}

/// Back up every save folder into a new folder under `<data root>\Backups`. A folder that
/// cannot be saved is reported in `failed` and the rest carry on.
pub fn backup_all(root: &DataRoot, db: &Db, cat: &Catalog, progress: &dyn Fn(Progress)) -> Result<Backup> {
    run(root, &save_sources(root, db, cat)?, progress)
}

fn run(root: &DataRoot, sources: &[SaveSource], progress: &dyn Fn(Progress)) -> Result<Backup> {
    if sources.is_empty() {
        bail!("There are no saves to back up yet.");
    }
    let created = now();
    let name = folder_name(&root.backups(), created, util::local_offset());
    let dir = root.backups().join(&name);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut items = Vec::new();
    let mut failed = Vec::new();
    for (i, s) in sources.iter().enumerate() {
        progress(Progress::new("backup", s.name.clone(), i, sources.len()));
        let out = dir.join(format!("{}.zip", file_name(&s.name)));
        match zip_folder(Path::new(&s.path), &out, &s.skip) {
            Ok(size) => items.push(BackupItem { name: s.name.clone(), from: s.path.clone(), size }),
            Err(e) => failed.push(format!("Could not save {}: {e}", s.name)),
        }
    }
    progress(Progress::new("done", format!("{} saved", items.len()), sources.len(), sources.len()));
    if items.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        bail!("{}", failed.join(" "));
    }
    let backup = Backup {
        name,
        created,
        size: items.iter().map(|i| i.size).sum(),
        path: path_str(&dir),
        items,
        failed,
    };
    std::fs::write(dir.join(MANIFEST), serde_json::to_string_pretty(&backup)?)?;
    Ok(backup)
}

/// "2026-10-10_1532" in local time, or "2026-10-10_1532_2" when that folder already exists.
fn folder_name(backups: &Path, created: i64, utc_offset: i64) -> String {
    let (y, mo, d, h, mi) = util::civil(created, utc_offset);
    let base = format!("{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}");
    let mut name = base.clone();
    let mut n = 2;
    while backups.join(&name).exists() {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// Past backups, newest first. Sizes come from the archives on disk, so a backup the player
/// trimmed by hand is shown as it is now.
pub fn list_backups(root: &DataRoot) -> Vec<Backup> {
    let Ok(rd) = std::fs::read_dir(root.backups()) else { return vec![] };
    let mut out: Vec<Backup> = rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).filter_map(|e| read_backup(&e.path())).collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then(b.name.cmp(&a.name)));
    out
}

fn read_backup(dir: &Path) -> Option<Backup> {
    let manifest: Option<Backup> =
        std::fs::read_to_string(dir.join(MANIFEST)).ok().and_then(|s| serde_json::from_str(&s).ok());
    let mut items: Vec<BackupItem> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let (stem, ext) = util::split_name(&path);
            if ext != ".zip" || !path.is_file() {
                return None;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            let known = manifest.as_ref().and_then(|m| m.items.iter().find(|i| file_name(&i.name).eq_ignore_ascii_case(&stem)));
            Some(BackupItem {
                name: known.map(|k| k.name.clone()).unwrap_or(stem),
                from: known.map(|k| k.from.clone()).unwrap_or_default(),
                size,
            })
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let created = manifest.as_ref().map(|m| m.created).unwrap_or_else(|| {
        std::fs::metadata(dir)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    });
    Some(Backup {
        name: dir.file_name()?.to_string_lossy().to_string(),
        created,
        size: items.iter().map(|i| i.size).sum(),
        path: path_str(dir),
        items,
        failed: manifest.map(|m| m.failed).unwrap_or_default(),
    })
}

/// Zip the contents of `dir` into `out` (via a `.part` file, so a cut-short run never leaves
/// something that looks complete). Returns the archive's size.
fn zip_folder(dir: &Path, out: &Path, skip: &[String]) -> Result<u64> {
    if !dir.is_dir() {
        bail!("its folder is no longer there.");
    }
    let part = out.with_extension("zip.part");
    let mut cmd = Command::new(install::system_tar());
    cmd.arg("-c").arg("--format").arg("zip");
    for s in skip {
        cmd.arg("--exclude").arg(s);
    }
    cmd.arg("-f").arg(&part).arg("-C").arg(dir).arg(".");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let res = cmd.output().context("Windows' tar.exe could not be started")?;
    if !res.status.success() {
        let _ = std::fs::remove_file(&part);
        eprintln!("backup of {} failed: {}", dir.display(), String::from_utf8_lossy(&res.stderr).trim());
        bail!("some files could not be read. Close the emulator and try again.");
    }
    std::fs::rename(&part, out)?;
    Ok(std::fs::metadata(out)?.len())
}

fn has_files(dir: &Path) -> bool {
    walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()).any(|e| e.file_type().is_file())
}

/// A name that is safe as a Windows file name.
fn file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '-' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() { "saves".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{AppSource, InstalledApp};
    use crate::util::TempDir;

    fn tar_list(zip: &Path) -> Vec<String> {
        let out = Command::new(install::system_tar()).arg("-tf").arg(zip).output().unwrap();
        String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim_start_matches("./").to_string()).collect()
    }

    #[test]
    fn backs_up_emulator_and_port_saves() {
        let t = TempDir::new("backups");
        let home = t.path().join("home");
        let env = |k: &str| match k {
            "USERPROFILE" => Some(path_str(&home)),
            "APPDATA" => Some(path_str(&home.join("AppData").join("Roaming"))),
            "LOCALAPPDATA" => Some(path_str(&home.join("AppData").join("Local"))),
            _ => None,
        };
        let root = DataRoot::open(t.path().join("root")).unwrap();
        let cat = Catalog::load(None).unwrap();
        let db = Db::open(Path::new(":memory:")).unwrap();

        // PCSX2 beside its memory cards, plus an old install in Documents.
        let pcsx2 = t.file("Emu/PCSX2/pcsx2-qt.exe", 10);
        t.write("Emu/PCSX2/memcards/Mcd001.ps2", b"card one");
        t.write("home/Documents/PCSX2/memcards/Mcd001.ps2", b"older card");
        db.upsert_emulator("pcsx2", "PCSX2", &path_str(&pcsx2), None, false, 1).unwrap();
        // Dolphin, portable: GameCube cards and a Wii NAND with an installed channel to skip.
        let dolphin = t.file("Emu/Dolphin/Dolphin.exe", 10);
        t.write("Emu/Dolphin/User/GC/MemoryCardA.USA.raw", b"gc");
        t.write("Emu/Dolphin/User/Wii/title/00010000/52534245/data/save.bin", b"wii save");
        t.file("Emu/Dolphin/User/Wii/title/00010001/48414445/content/00000001.app", 4096);
        db.upsert_emulator("dolphin", "Dolphin", &path_str(&dolphin), None, false, 1).unwrap();
        // DuckStation with an empty memcards folder: nothing to save.
        let duck = t.file("Emu/DuckStation/duckstation-qt.exe", 10);
        std::fs::create_dir_all(t.path().join("Emu/DuckStation/memcards")).unwrap();
        db.upsert_emulator("duckstation", "DuckStation", &path_str(&duck), None, false, 1).unwrap();
        // An installed port with saves in persist.
        let port = &cat.ports[0];
        let app = InstalledApp {
            id: port.id.clone(),
            name: port.name.clone(),
            kind: "port".into(),
            version: "v1".into(),
            installed_at: 1,
            exe: "game.exe".into(),
            configure_exe: None,
            size: 1,
            source: AppSource { repo: None, tag: None, asset: None, sha256: String::new(), verified: false, archive: String::new() },
            versions: vec![],
        };
        let app_dir = root.apps().join(&port.id);
        std::fs::create_dir_all(app_dir.join("persist").join("saves")).unwrap();
        std::fs::write(app_dir.join("persist").join("saves").join("slot1.sav"), b"port save").unwrap();
        std::fs::write(app_dir.join("app.json"), serde_json::to_string(&app).unwrap()).unwrap();

        let sources = sources_with(&root, &db, &cat, &env).unwrap();
        let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["PCSX2", "PCSX2 2", "Dolphin (GC)", "Dolphin (Wii)", port.game.as_str()]);
        assert_eq!(sources[3].skip, vec!["content".to_string(), "sd.raw".to_string()]);

        let b = run(&root, &sources, &|_| {}).unwrap();
        assert!(b.failed.is_empty(), "{:?}", b.failed);
        assert_eq!(b.items.len(), 5);
        assert!(b.items.iter().all(|i| i.size > 0));
        let dir = PathBuf::from(&b.path);
        assert!(dir.starts_with(root.backups()));
        let wii = tar_list(&dir.join("Dolphin (Wii).zip"));
        assert!(wii.iter().any(|f| f.ends_with("data/save.bin")), "{wii:?}");
        assert!(!wii.iter().any(|f| f.contains("content")), "installed channel left out: {wii:?}");
        assert!(tar_list(&dir.join("PCSX2.zip")).contains(&"Mcd001.ps2".to_string()));
        assert!(!dir.read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".part")));

        // A second run gets its own folder; nothing is overwritten.
        let again = run(&root, &sources, &|_| {}).unwrap();
        assert_ne!(again.path, b.path);

        let list = list_backups(&root);
        assert_eq!(list.len(), 2);
        let first = list.iter().find(|x| x.name == b.name).unwrap();
        assert_eq!(first.items.len(), 5);
        assert_eq!(first.size, b.size);
        assert!(first.items.iter().any(|i| i.name == "Dolphin (Wii)" && i.from.ends_with("Wii")));
    }

    #[test]
    fn nothing_to_back_up() {
        let t = TempDir::new("backups-empty");
        let root = DataRoot::open(t.path().join("root")).unwrap();
        assert!(run(&root, &[], &|_| {}).is_err());
        assert!(list_backups(&root).is_empty());
        let missing = SaveSource { name: "Gone".into(), path: path_str(&t.path().join("nope")), skip: vec![] };
        let err = run(&root, &[missing], &|_| {}).unwrap_err().to_string();
        assert_eq!(err, "Could not save Gone: its folder is no longer there.");
        assert!(list_backups(&root).is_empty(), "a run that saved nothing leaves nothing behind");
    }

    #[test]
    fn folder_names() {
        let t = TempDir::new("backups-names");
        let at = 1_704_067_200 + 15 * 3600 + 32 * 60; // 2024-01-01 15:32 UTC
        assert_eq!(folder_name(t.path(), at, 0), "2024-01-01_1532");
        assert_eq!(folder_name(t.path(), at, -16 * 3600), "2023-12-31_2332");
        std::fs::create_dir_all(t.path().join("2024-01-01_1532")).unwrap();
        std::fs::create_dir_all(t.path().join("2024-01-01_1532_2")).unwrap();
        assert_eq!(folder_name(t.path(), at, 0), "2024-01-01_1532_3");
    }

    #[test]
    fn safe_file_names() {
        assert_eq!(file_name("Dolphin (Wii)"), "Dolphin (Wii)");
        assert_eq!(file_name("Zelda: Ocarina / Majora?"), "Zelda- Ocarina - Majora-");
        assert_eq!(file_name("Dots..."), "Dots");
        assert_eq!(file_name("  "), "saves");
    }
}
