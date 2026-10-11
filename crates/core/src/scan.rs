//! Library scanning.
//!
//! Existing setups are *adopted in place*: we walk each source folder, recognise emulator
//! executables and game files, and record where they are. Nothing under a source is moved or
//! modified. The same game found several times (ISO + extracted folder + the archive it came
//! in) becomes one library entry with several copies, which also powers the storage report.

use crate::catalog::Catalog;
use crate::db::{Db, GameUpsert};
use crate::disc::{self, NintendoDisc};
use crate::install;
use crate::model::{Copy, Progress, ScanSummary};
use crate::paths::DataRoot;
use crate::util::{self, clean_title, norm_key, now, path_str, region_of, split_name};
use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::{DirEntry, WalkDir};

/// Emulator internals and other folders that never hold games.
pub(crate) const SKIP_DIRS: &[&str] = &[
    "dev_flash", "dev_flash2", "dev_flash3", "dev_hdd1", "dev_bdvd", "dev_usb000", "cache", "shaderlog",
    "shadercache", "shaders", "qtplugins", "languages", "translations", "resources", "sys", "bios", "memcards",
    "sstates", "savestates", "textures", "cheats", "logs", "captures", "recordings", "snaps", "screenshots",
    "inputprofiles", "gamesettings", "patches", "videos", "ppu_progs", "spu_progs", "icons", "guiconfigs",
    "config", "licenses", "platforms", "imageformats", "styles", "mediaservice", "scripting", "docs", "battery",
    "stateslots", "nand", "sdmc", "load", "maps", "themes", "covers", "$recycle.bin", "system volume information",
    "node_modules", "reclaim", "rpcs3_old", "old", "backup", "backups", "_internal", "qt6", "sounds", "test",
];

pub(crate) const ARCHIVE_EXTS: &[&str] = &[".zip", ".7z", ".rar"];

/// A version in an emulator's folder name, with RPCS3/DuckStation-style build numbers kept:
/// `rpcs3-v0.0.34-17210-0fd6956b_win64` -> `0.0.34-17210` (see `updates::Version`).
static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)v?(\d+\.\d+(?:\.\d+)*(?:-\d{3,})?)").unwrap());

#[derive(Debug, Clone)]
pub struct FoundEmu {
    pub def_id: String,
    pub exe: PathBuf,
    pub version: Option<String>,
    pub managed: bool,
}

#[derive(Debug)]
struct Item {
    platform: String,
    title: String,
    region: Option<String>,
    serial: Option<String>,
    /// What to hand the emulator (file, EBOOT.BIN, ...).
    launch: PathBuf,
    /// The thing on disk (file or game folder).
    path: PathBuf,
    format: String,
    size: u64,
    archive: bool,
}

struct Walk<'a> {
    cat: &'a Catalog,
    emus: Vec<FoundEmu>,
    items: Vec<Item>,
    archives: Vec<(PathBuf, u64)>,
    registered: HashSet<String>,
    unclassified: usize,
}

fn lower(p: &Path) -> String {
    path_str(p).to_lowercase().trim_end_matches(['\\', '/']).to_string()
}

fn is_skipped(e: &DirEntry) -> bool {
    if !e.file_type().is_dir() || e.depth() == 0 {
        return false;
    }
    let name = e.file_name().to_string_lossy().to_lowercase();
    name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) || name.ends_with("_old")
}

/// A folder path in one spelling, for comparing folders: lower case, one kind of slash.
fn own_key(p: &Path) -> String {
    path_str(p).replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn version_from_dirs(exe: &Path) -> Option<String> {
    exe.ancestors().skip(1).take(2).find_map(|d| {
        let n = d.file_name()?.to_string_lossy().to_string();
        VERSION_RE.captures(&n).map(|c| c[1].to_string())
    })
}

impl<'a> Walk<'a> {
    /// A PS3 disc folder, PS3 HDD game, or PS4 game folder. Returns true if `dir` is one
    /// (so the walker does not descend into thousands of game files).
    fn game_folder(&mut self, dir: &Path) -> bool {
        let disc_sfo = dir.join("PS3_GAME").join("PARAM.SFO");
        if disc_sfo.is_file() {
            if let Some(sfo) = disc::read_sfo(&disc_sfo) {
                let eboot = dir.join("PS3_GAME").join("USRDIR").join("EBOOT.BIN");
                self.push_folder("ps3", dir, eboot, &sfo, "folder");
            }
            return true;
        }
        let sfo_path = dir.join("PARAM.SFO");
        let parent_is_game = dir.parent().and_then(|p| p.file_name()).is_some_and(|n| n.eq_ignore_ascii_case("game"));
        if parent_is_game && sfo_path.is_file() {
            if let Some(sfo) = disc::read_sfo(&sfo_path) {
                let eboot = dir.join("USRDIR").join("EBOOT.BIN");
                // HG covers apps too (digital manuals, demos' launchers); keep actual games.
                let title = sfo.get("TITLE").unwrap_or_default().to_lowercase();
                if sfo.get("CATEGORY") == Some("HG") && eboot.is_file() && !title.contains("manual") {
                    self.push_folder("ps3", dir, eboot, &sfo, "hdd");
                }
            }
            return true;
        }
        // PS5 dumps describe themselves in sce_sys/param.json.
        let ps5_json = dir.join("sce_sys").join("param.json");
        if ps5_json.is_file() && dir.join("eboot.bin").is_file() {
            if let Some(v) = std::fs::read_to_string(&ps5_json).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
                let lang = v.pointer("/localizedParameters/defaultLanguage").and_then(|x| x.as_str()).unwrap_or("en-US");
                let title = v
                    .pointer(&format!("/localizedParameters/{lang}/titleName"))
                    .or_else(|| v.pointer("/localizedParameters/en-US/titleName"))
                    .and_then(|x| x.as_str())
                    .map(String::from);
                let mut sfo = disc::Sfo::default();
                if let Some(t) = title {
                    sfo.values.insert("TITLE".into(), t);
                }
                if let Some(id) = v.get("titleId").and_then(|x| x.as_str()) {
                    sfo.values.insert("TITLE_ID".into(), id.into());
                }
                self.push_folder("ps5", dir, dir.join("eboot.bin"), &sfo, "folder");
            }
            return true;
        }
        let ps4_sfo = dir.join("sce_sys").join("param.sfo");
        if ps4_sfo.is_file() && dir.join("eboot.bin").is_file() {
            if let Some(sfo) = disc::read_sfo(&ps4_sfo) {
                let eboot = dir.join("eboot.bin");
                self.push_folder("ps4", dir, eboot, &sfo, "folder");
            }
            return true;
        }
        false
    }

    fn push_folder(&mut self, platform: &str, dir: &Path, launch: PathBuf, sfo: &disc::Sfo, format: &str) {
        let title = sfo.get("TITLE").map(|t| t.replace('\n', " ")).unwrap_or_else(|| {
            dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        });
        let serial = sfo.get("TITLE_ID").map(|s| s.to_string());
        self.items.push(Item {
            platform: platform.into(),
            title: util::tidy_case(&clean_title(&title)),
            region: region_from_serial(serial.as_deref()),
            serial,
            launch,
            path: dir.to_path_buf(),
            format: format.into(),
            size: util::dir_size(dir),
            archive: false,
        });
    }

    fn file(&mut self, path: &Path, size: u64) {
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (stem, ext) = split_name(path);
        if ext == ".exe" {
            if let Some(def) = self.cat.match_emulator_exe(&file_name) {
                self.emus.push(FoundEmu { def_id: def.id.clone(), exe: path.to_path_buf(), version: version_from_dirs(path), managed: false });
            }
            return;
        }
        if file_name.eq_ignore_ascii_case("games.yml") && path.with_file_name("rpcs3.exe").exists() {
            self.read_rpcs3_games(path);
            return;
        }
        if ARCHIVE_EXTS.contains(&ext.as_str()) {
            self.archives.push((path.to_path_buf(), size));
            return;
        }
        if !self.cat.is_rom_ext(&ext) || size < 16 * 1024 {
            return;
        }
        let Some((platform, serial)) = self.classify(path, &ext) else {
            self.unclassified += 1;
            return;
        };
        self.items.push(Item {
            platform,
            title: clean_title(&stem),
            region: region_of(&stem),
            serial,
            launch: path.to_path_buf(),
            path: path.to_path_buf(),
            format: ext.trim_start_matches('.').to_string(),
            size,
            archive: false,
        });
    }

    /// Pick a platform for a file: unique extension, else the nearest folder name that hints a
    /// platform, else (GameCube vs Wii) the disc header. Ambiguous files are skipped, not guessed.
    fn classify(&self, path: &Path, ext: &str) -> Option<(String, Option<String>)> {
        let mut cands: Vec<&str> = self.cat.platforms_for_ext(ext);
        if cands.len() > 1 {
            for seg in path.ancestors().skip(1) {
                let Some(name) = seg.file_name() else { continue };
                let name = name.to_string_lossy();
                let hit: Vec<&str> = cands.iter().copied().filter(|p| self.cat.hint_matches(p, &name)).collect();
                if !hit.is_empty() {
                    cands = hit;
                    break;
                }
            }
        }
        let nintendo = cands.iter().all(|p| *p == "gc" || *p == "wii");
        if nintendo
            && let Some((kind, id)) = disc::nintendo_disc(path) {
                let p = if kind == NintendoDisc::Wii { "wii" } else { "gc" };
                if cands.contains(&p) {
                    return Some((p.into(), id));
                }
            }
        (cands.len() == 1).then(|| (cands[0].to_string(), None))
    }

    fn read_rpcs3_games(&mut self, yml: &Path) {
        let Ok(text) = std::fs::read_to_string(yml) else { return };
        for line in text.lines() {
            if let Some((_, v)) = line.split_once(':') {
                let v = v.trim().trim_matches('"').replace('/', "\\");
                if v.len() > 3 {
                    self.registered.insert(v.to_lowercase().trim_end_matches('\\').to_string());
                }
            }
        }
    }
}

fn region_from_serial(serial: Option<&str>) -> Option<String> {
    let s = serial?;
    let c = s.chars().nth(2)?;
    Some(
        match c {
            'U' => "USA",
            'E' => "EUR",
            'J' | 'P' => "JPN",
            'A' => "ASIA",
            'K' => "KOR",
            _ => return None,
        }
        .into(),
    )
}

fn format_rank(format: &str) -> u8 {
    match format {
        "folder" | "m3u" => 0,
        "hdd" | "cue" => 1,
        "chd" | "rvz" => 2,
        "iso" | "gcm" | "wbfs" => 3,
        _ => 4,
    }
}

pub fn scan(root: &DataRoot, cat: &Catalog, db: &Db, progress: &dyn Fn(Progress)) -> Result<ScanSummary> {
    let started = std::time::Instant::now();
    let seen = now();
    let sources = db.sources()?;
    let mut w = Walk { cat, emus: vec![], items: vec![], archives: vec![], registered: HashSet::new(), unclassified: 0 };
    // OnionRing's own folders, for a data folder inside a source folder: its installs are added
    // below as managed apps, and its downloads, art and backups are not games.
    let own: Vec<String> = [root.apps(), root.downloads(), root.media(), root.cache(), root.reclaim(), root.backups()]
        .iter()
        .map(|p| own_key(p))
        .collect();

    for (i, src) in sources.iter().enumerate() {
        if !src.exists {
            continue;
        }
        progress(Progress::new("walk", src.path.clone(), i, sources.len()));
        let mut game_dirs: Vec<PathBuf> = Vec::new();
        let mut n = 0usize;
        let it = WalkDir::new(&src.path).follow_links(false).into_iter().filter_entry(|e| {
            if is_skipped(e) || (e.file_type().is_dir() && own.contains(&own_key(e.path()))) {
                return false;
            }
            if e.file_type().is_dir() && e.depth() > 0 {
                let dir = e.path();
                if dir.join("PS3_GAME").is_dir() || dir.join("PARAM.SFO").is_file() || dir.join("sce_sys").is_dir() {
                    game_dirs.push(dir.to_path_buf());
                    return false;
                }
            }
            true
        });
        for entry in it.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            n += 1;
            if n.is_multiple_of(500) {
                progress(Progress::new("walk", path_str(entry.path()), i, sources.len()));
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            w.file(entry.path(), size);
        }
        for d in game_dirs {
            progress(Progress::new("walk", path_str(&d), i, sources.len()));
            w.game_folder(&d);
        }
    }

    // Emulators and ports OnionRing installed itself.
    let installed = install::installed_apps(root);
    for app in &installed {
        if app.kind == "emulator"
            && let Some(exe) = app.exe_path(root) {
                w.emus.push(FoundEmu { def_id: app.id.clone(), exe, version: Some(app.version.clone()), managed: true });
            }
    }

    // ---- emulators --------------------------------------------------------------------------
    progress(Progress::new("organise", "Emulators", 0, 1));
    let mut emu_ids: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new(); // platform -> [(instance id, dir)]
    let mut used: HashSet<String> = HashSet::new();
    w.emus.sort_by(|a, b| b.managed.cmp(&a.managed).then(a.exe.cmp(&b.exe)));
    for e in &w.emus {
        let Some(def) = cat.emulator(&e.def_id) else { continue };
        let mut id = def.id.clone();
        let mut k = 2;
        while used.contains(&id) {
            id = format!("{}-{k}", def.id);
            k += 1;
        }
        used.insert(id.clone());
        let name = if id == def.id { def.name.clone() } else { format!("{} ({k})", def.name) };
        db.upsert_emulator(&id, &name, &path_str(&e.exe), e.version.as_deref(), e.managed, seen)?;
        let dir = e.exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        for p in &def.platforms {
            emu_ids.entry(p.clone()).or_default().push((id.clone(), dir.clone()));
        }
    }
    // Catalog order expresses preference (maintained emulators first).
    let order = |id: &str| cat.emulators.iter().position(|d| id.starts_with(&d.id)).unwrap_or(usize::MAX);
    for list in emu_ids.values_mut() {
        list.sort_by_key(|(id, _)| order(id));
    }
    let pick_emu = |platform: &str, path: &Path| -> Option<String> {
        let list = emu_ids.get(platform)?;
        let lp = lower(path);
        let mut best: Option<(&String, usize)> = None;
        for (id, dir) in list {
            let ld = lower(dir);
            // Prefer the emulator whose folder (or its parent) contains the game.
            let parent = dir.parent().map(lower).unwrap_or_default();
            if lp.starts_with(&ld) {
                return Some(id.clone());
            }
            if !parent.is_empty() && lp.starts_with(&parent) && best.is_none_or(|(_, l)| parent.len() > l) {
                best = Some((id, parent.len()));
            }
        }
        best.map(|(id, _)| id.clone()).or_else(|| list.first().map(|(id, _)| id.clone()))
    };

    // ---- group copies into games ------------------------------------------------------------
    progress(Progress::new("organise", "Games", 0, 1));
    let mut groups: HashMap<String, Vec<Item>> = HashMap::new();
    for it in w.items.drain(..) {
        let key = format!("{}:{}", it.platform, norm_key(&it.title));
        groups.entry(key).or_default().push(it);
    }

    // Archives join the game they unpack to (same name, same folder tree); otherwise they
    // become a "packed" game when the folder makes the platform clear.
    for (path, size) in w.archives.drain(..) {
        let (stem, ext) = split_name(&path);
        let nk = norm_key(&stem);
        let parent = path.parent().map(lower).unwrap_or_default();
        let target = groups
            .iter()
            .find(|(k, items)| k.split_once(':').is_some_and(|(_, t)| t == nk) && items.iter().any(|i| lower(&i.path).starts_with(&parent)))
            .map(|(k, _)| k.clone());
        let item = |platform: String| Item {
            platform,
            title: clean_title(&stem),
            region: region_of(&stem),
            serial: None,
            launch: path.clone(),
            path: path.clone(),
            format: ext.trim_start_matches('.').to_string(),
            size,
            archive: true,
        };
        if let Some(k) = target {
            let platform = groups[&k][0].platform.clone();
            groups.get_mut(&k).unwrap().push(item(platform));
            continue;
        }
        let hinted: Option<String> = path.ancestors().skip(1).find_map(|seg| {
            let name = seg.file_name()?.to_string_lossy().to_string();
            let hits: Vec<&str> = cat
                .platforms
                .iter()
                .filter(|p| !p.extensions.is_empty() && cat.hint_matches(&p.id, &name))
                .map(|p| p.id.as_str())
                .collect();
            (hits.len() == 1).then(|| hits[0].to_string())
        });
        if let Some(p) = hinted {
            groups.entry(format!("{p}:{nk}")).or_default().push(item(p));
        }
    }

    let mut summary = ScanSummary::default();
    let total = groups.len();
    for (i, (key, mut items)) in groups.into_iter().enumerate() {
        if i % 25 == 0 {
            progress(Progress::new("save", key.clone(), i, total));
        }
        for it in items.iter_mut() {
            let lp = lower(&it.path);
            it.format = it.format.to_lowercase();
            if w.registered.contains(&lp) {
                it.format = format!("{}+", it.format); // marker, stripped below
            }
        }
        items.sort_by(|a, b| {
            let ra = a.format.ends_with('+');
            let rb = b.format.ends_with('+');
            rb.cmp(&ra)
                .then(a.archive.cmp(&b.archive))
                .then(format_rank(a.format.trim_end_matches('+')).cmp(&format_rank(b.format.trim_end_matches('+'))))
                .then(a.path.components().count().cmp(&b.path.components().count()))
                .then(a.path.cmp(&b.path))
        });
        let primary = &items[0];
        let packed = primary.archive;
        let serial = items.iter().find_map(|i| i.serial.clone());
        let region = items.iter().find_map(|i| i.region.clone());
        // Prefer the title from a folder's PARAM.SFO only if the file name is unhelpful.
        let title = items.iter().find(|i| !i.archive && i.format.trim_end_matches('+') != "folder").map(|i| i.title.clone())
            .unwrap_or_else(|| primary.title.clone());
        let up = GameUpsert {
            key: key.clone(),
            title,
            platform: primary.platform.clone(),
            kind: "rom".into(),
            launch_path: path_str(&primary.launch),
            emulator: pick_emu(&primary.platform, &primary.path),
            app_id: None,
            serial,
            region,
            size: primary.size as i64,
            packed,
        };
        let (_, is_new) = db.upsert_game(&up, seen)?;
        if is_new {
            summary.new_games += 1;
        }
        let copies: Vec<Copy> = items
            .iter()
            .enumerate()
            .map(|(n, it)| Copy {
                path: path_str(&it.path),
                registered: it.format.ends_with('+'),
                format: it.format.trim_end_matches('+').to_string(),
                size: it.size as i64,
                archive: it.archive,
                primary: n == 0,
            })
            .collect();
        summary.copies += copies.len();
        db.replace_copies(&key, &copies)?;
        summary.games += 1;
    }

    // ---- installed ports --------------------------------------------------------------------
    for app in installed.iter().filter(|a| a.kind == "port") {
        let Some(exe) = app.exe_path(root) else { continue };
        let def = cat.port(&app.id);
        let key = format!("port:{}", app.id);
        let up = GameUpsert {
            key: key.clone(),
            title: def.map(|d| d.game.clone()).unwrap_or_else(|| app.name.clone()),
            platform: "port".into(),
            kind: "port".into(),
            launch_path: path_str(&exe),
            emulator: None,
            app_id: Some(app.id.clone()),
            serial: None,
            region: None,
            size: app.size as i64,
            packed: false,
        };
        let (_, is_new) = db.upsert_game(&up, seen)?;
        summary.new_games += is_new as usize;
        summary.games += 1;
    }

    db.sweep_unseen("rom", seen)?;
    db.sweep_unseen("port", seen)?;
    let online: Vec<String> = sources.iter().filter(|s| s.exists).map(|s| s.path.to_lowercase()).collect();
    // A game under an online source that this scan no longer recognised is gone for good.
    let gone = |p: &str| {
        let lp = p.to_lowercase();
        online.iter().any(|s| lp.starts_with(s.as_str()))
    };
    db.forget_missing(gone)?;
    summary.missing = db.games()?.iter().filter(|g| g.missing).count();
    summary.emulators = used.len();
    summary.millis = started.elapsed().as_millis();
    progress(Progress::new("done", format!("{} games", summary.games), 1, 1));
    if w.unclassified > 0 {
        eprintln!("scan: {} files skipped because their platform was ambiguous", w.unclassified);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_from_folder_names() {
        let v = |p: &str| version_from_dirs(Path::new(p));
        assert_eq!(v(r"G:\Emulation\PS3\rpcs3-v0.0.34-17210-0fd6956b_win64\rpcs3.exe").as_deref(), Some("0.0.34-17210"));
        assert_eq!(v(r"G:\Emulation\PS2\pcsx2-v2.3.70-windows-x64-Qt\pcsx2-qt.exe").as_deref(), Some("2.3.70"));
        assert_eq!(v(r"G:\Emulation\DS\desmume-0.9.13-win64\DeSmuME_0.9.13_x64.exe").as_deref(), Some("0.9.13"));
        assert_eq!(v(r"G:\Emulation\GC\Dolphin-x64\Dolphin.exe"), None);
    }
}
