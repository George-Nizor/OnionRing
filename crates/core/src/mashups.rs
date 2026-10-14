//! Mashups: mods that put one game inside another ("Minecraft in Skyrim", "Minecraft in Elden
//! Ring"). The player owns the host game; OnionRing installs the mod from its GitHub releases
//! (through `install`, like a port), checks the files only the player can get (a script
//! extender, a library behind a site login), and starts it.
//!
//! A mashup that has to sit inside the game's folder (`deploy: "game"`) is copied there from
//! `Apps\<id>\current`. Every file written is recorded with its hash, and a file that was already
//! there is moved aside first:
//! ```text
//! Apps\<id>\deployed.json    the folder it went to, each file written, what was replaced
//! Apps\<id>\replaced\        the game's own files that were in the way
//! ```
//! Removing the mashup deletes exactly those files and puts the replaced ones back. A file the
//! player edited after it was written (a settings file) is left as they made it on update.

use crate::catalog::{Catalog, HostDef, MashupDef, NeedDef};
use crate::launch::Plan;
use crate::paths::DataRoot;
use crate::util::{path_str, sha256_file};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// The host game's folder: the one the player chose, else where Steam installed it.
pub fn host_dir(h: &HostDef) -> Option<PathBuf> {
    let has_game = |d: &PathBuf| d.join(&h.exe).is_file();
    if let Some(d) = h.dir.as_deref().map(PathBuf::from).filter(has_game) {
        return Some(d);
    }
    h.steam.and_then(crate::steam::app_dir).filter(has_game)
}

/// Where a mashup's files go when it is copied into the game; `None` for one that stays in its
/// own folder.
pub fn deploy_dir(def: &MashupDef, game_dir: &Path) -> Option<PathBuf> {
    (def.deploy == "game").then(|| match &def.into {
        Some(sub) => game_dir.join(sub),
        None => game_dir.to_path_buf(),
    })
}

fn need_met(game_dir: &Path, need: &NeedDef) -> bool {
    need.paths.iter().any(|p| {
        let p = game_dir.join(p);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.contains('*') || name.contains('?') {
            p.parent().is_some_and(|dir| !crate::checks::glob(dir, &name).is_empty())
        } else {
            p.is_file()
        }
    })
}

/// The host game as the UI shows it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostState {
    pub id: String,
    pub name: String,
    /// Where the game is, for an "open folder" button (never shown as text).
    pub dir: Option<String>,
    /// The player chose the folder (rather than OnionRing finding it in Steam).
    pub chosen: bool,
    pub steam: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NeedState {
    pub what: String,
    pub ok: bool,
    pub url: Option<String>,
    pub help: String,
}

/// What stands between the player and playing a mashup.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MashupState {
    pub id: String,
    pub host: HostState,
    /// Empty when the host game isn't found (nothing can be checked).
    pub needs: Vec<NeedState>,
    /// Its files are in the game's folder now.
    pub deployed: bool,
}

pub fn host_state(h: &HostDef) -> HostState {
    let dir = host_dir(h);
    let chosen = dir.is_some() && h.dir.as_deref().map(PathBuf::from) == dir;
    HostState { id: h.id.clone(), name: h.name.clone(), dir: dir.as_deref().map(path_str), chosen, steam: h.steam }
}

pub fn state(root: &DataRoot, cat: &Catalog, def: &MashupDef) -> MashupState {
    let host = cat.host(&def.host);
    let dir = host.and_then(host_dir);
    let needs = match &dir {
        Some(d) => def.needs.iter().map(|n| NeedState { what: n.what.clone(), ok: need_met(d, n), url: n.url.clone(), help: n.help.clone() }).collect(),
        None => vec![],
    };
    let host = host.map(host_state).unwrap_or_else(|| HostState { id: def.host.clone(), name: def.host.clone(), dir: None, chosen: false, steam: None });
    MashupState { id: def.id.clone(), host, needs, deployed: manifest_path(root, &def.id).is_file() }
}

pub fn states(root: &DataRoot, cat: &Catalog) -> Vec<MashupState> {
    cat.mashups.iter().map(|m| state(root, cat, m)).collect()
}

/// How to start an installed mashup. `app_exe` is its program inside its own install folder
/// (for mashups that start from there).
pub fn plan(cat: &Catalog, def: &MashupDef, app_exe: &str, user_args: Option<&[String]>) -> Result<Plan> {
    let host = cat.host(&def.host).with_context(|| format!("{} adds to a game OnionRing doesn't know ({}).", def.name, def.host))?;
    let game = host_dir(host).with_context(|| {
        format!("{} needs {}, which isn't installed. Install it through Steam, or choose its folder in Get more, Mashups.", def.name, host.name)
    })?;
    let missing: Vec<&str> = def.needs.iter().filter(|n| !need_met(&game, n)).map(|n| n.what.as_str()).collect();
    if !missing.is_empty() {
        bail!("{} also needs {} in your {} folder. See Get more, Mashups.", def.name, missing.join(" and "), host.name);
    }
    let program = match def.launch.from.as_str() {
        "game" => game.join(&def.launch.exe),
        _ => PathBuf::from(app_exe),
    };
    if !program.is_file() {
        bail!("The program that starts {} is missing. Reinstall it from Get more, Mashups.", def.name);
    }
    let game_exe = path_str(&game.join(&host.exe));
    let game_dir = path_str(&game);
    let args = user_args
        .unwrap_or(&def.launch.args)
        .iter()
        .map(|a| a.replace("{game_exe}", &game_exe).replace("{game_dir}", &game_dir))
        .collect();
    let cwd = program.parent().map(Path::to_path_buf).unwrap_or(game);
    Ok(Plan { program, args, cwd })
}

// ---- copying into the game -------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Deployed {
    /// The folder the files were copied into.
    dir: String,
    files: Vec<DeployedFile>,
    /// Folders that had to be made (relative), removed again if empty.
    #[serde(default)]
    made: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct DeployedFile {
    /// Relative to `dir`.
    path: String,
    /// What OnionRing wrote, so a file the player changed since can be told apart.
    sha256: String,
    /// A file of the game's own was there first; it is kept under `replaced\`.
    replaced: bool,
}

fn manifest_path(root: &DataRoot, id: &str) -> PathBuf {
    root.apps().join(id).join("deployed.json")
}

fn replaced_dir(root: &DataRoot, id: &str) -> PathBuf {
    root.apps().join(id).join("replaced")
}

fn load(root: &DataRoot, id: &str) -> Option<Deployed> {
    serde_json::from_str(&std::fs::read_to_string(manifest_path(root, id)).ok()?).ok()
}

/// Only plain relative paths (no `..`, no drive) are ever joined onto the game's folder.
fn plain(rel: &Path) -> bool {
    rel.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Copy a mashup's files from `source` (its install folder) into `target` (inside the game).
pub fn deploy(root: &DataRoot, id: &str, source: &Path, target: &Path, progress: &dyn Fn(&str)) -> Result<()> {
    if !target.is_dir() {
        bail!("{} doesn't exist.", target.display());
    }
    let mut prev = load(root, id).unwrap_or_default();
    // The game moved (or was reinstalled elsewhere): take the old copy back out first.
    if !prev.dir.is_empty() && !Path::new(&prev.dir).eq(target) {
        undeploy(root, id)?;
        prev = Deployed::default();
    }
    let before: HashMap<String, DeployedFile> = prev.files.iter().map(|f| (f.path.to_lowercase(), f.clone())).collect();
    let mut made: Vec<String> = prev.made.clone();
    let mut files = Vec::new();
    let backup = replaced_dir(root, id);

    for e in walkdir::WalkDir::new(source).min_depth(1).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        let rel = e.path().strip_prefix(source)?.to_path_buf();
        if !plain(&rel) {
            continue;
        }
        let key = path_str(&rel);
        let dest = target.join(&rel);
        progress(&format!("Adding {key}"));
        let new_sha = sha256_file(e.path())?;
        let mut replaced = false;
        match before.get(&key.to_lowercase()) {
            Some(old) => {
                replaced = old.replaced;
                // The player changed this file since it was written: leave theirs.
                if dest.is_file() && sha256_file(&dest)? != old.sha256 {
                    files.push(old.clone());
                    continue;
                }
            }
            None if dest.exists() => {
                let keep = backup.join(&rel);
                std::fs::create_dir_all(keep.parent().unwrap())?;
                std::fs::copy(&dest, &keep).with_context(|| format!("keeping a copy of {}", dest.display()))?;
                replaced = true;
            }
            None => {}
        }
        // Remember each folder that has to be made, top-down.
        let mut missing: Vec<PathBuf> = rel.ancestors().skip(1).filter(|a| !a.as_os_str().is_empty() && !target.join(a).is_dir()).map(Path::to_path_buf).collect();
        missing.reverse();
        for m in missing {
            std::fs::create_dir(target.join(&m))?;
            made.push(path_str(&m));
        }
        std::fs::copy(e.path(), &dest).with_context(|| format!("copying {}", dest.display()))?;
        files.push(DeployedFile { path: key, sha256: new_sha, replaced });
    }

    // Files the old version had and this one doesn't.
    let now: std::collections::HashSet<String> = files.iter().map(|f| f.path.to_lowercase()).collect();
    for old in prev.files.iter().filter(|f| !now.contains(&f.path.to_lowercase())) {
        take_back(target, &backup, old)?;
    }
    remove_empty(target, &mut made);

    let m = Deployed { dir: path_str(target), files, made };
    std::fs::write(manifest_path(root, id), serde_json::to_string_pretty(&m)?)?;
    Ok(())
}

/// Delete one written file, putting back the game's own file if it replaced one.
fn take_back(target: &Path, backup: &Path, f: &DeployedFile) -> Result<()> {
    let rel = Path::new(&f.path);
    if !plain(rel) {
        return Ok(());
    }
    let dest = target.join(rel);
    if dest.is_file() {
        std::fs::remove_file(&dest).with_context(|| format!("removing {}", dest.display()))?;
    }
    let kept = backup.join(rel);
    if f.replaced && kept.is_file() {
        std::fs::rename(&kept, &dest).or_else(|_| std::fs::copy(&kept, &dest).map(|_| ()))?;
    }
    Ok(())
}

/// Remove folders OnionRing made that are empty now, deepest first. Ones still in use stay listed.
fn remove_empty(target: &Path, made: &mut Vec<String>) {
    made.sort_by_key(|m| std::cmp::Reverse(Path::new(m).components().count()));
    made.dedup();
    made.retain(|m| {
        let d = target.join(m);
        let empty = std::fs::read_dir(&d).map(|mut r| r.next().is_none()).unwrap_or(false);
        !(plain(Path::new(m)) && empty && std::fs::remove_dir(&d).is_ok()) && d.is_dir()
    });
}

/// Take a mashup back out of the game, exactly as it was before.
pub fn undeploy(root: &DataRoot, id: &str) -> Result<()> {
    let Some(mut m) = load(root, id) else { return Ok(()) };
    let target = PathBuf::from(&m.dir);
    let backup = replaced_dir(root, id);
    if target.is_dir() {
        for f in &m.files {
            take_back(&target, &backup, f)?;
        }
        remove_empty(&target, &mut m.made);
    }
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::remove_file(manifest_path(root, id))?;
    Ok(())
}

/// Uninstall a mashup: out of the game, then its install folder.
pub fn remove(root: &DataRoot, id: &str) -> Result<()> {
    undeploy(root, id)?;
    let dir = root.apps().join(id);
    let current = dir.join("current");
    if std::fs::symlink_metadata(&current).is_ok() {
        let _ = junction::delete(&current);
        let _ = std::fs::remove_dir(&current);
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn deploys_and_puts_the_game_back() {
        let t = TempDir::new("mashup");
        let root = DataRoot::open(t.path().join("root")).unwrap();
        let game = t.path().join("Skyrim");
        let data = game.join("Data");
        // The game's own files, one of which the mod replaces.
        t.write("Skyrim/Data/Skyrim.esm", b"game");
        t.write("Skyrim/Data/SKSE/Plugins/Shared.ini", b"game's own");

        let v1 = t.path().join("root/Apps/skycraft/v1");
        t.write("root/Apps/skycraft/v1/SKSE/Plugins/SkyCraft.dll", b"dll v1");
        t.write("root/Apps/skycraft/v1/SKSE/Plugins/SkyCraft.ini", b"ini v1");
        t.write("root/Apps/skycraft/v1/SKSE/Plugins/Shared.ini", b"mod's");
        t.write("root/Apps/skycraft/v1/SKSE/Plugins/SkyCraft/Bundle/old.txt", b"only in v1");
        deploy(&root, "skycraft", &v1, &data, &|_| {}).unwrap();
        assert_eq!(read(&data.join("SKSE/Plugins/SkyCraft.dll")), "dll v1");
        assert_eq!(read(&data.join("SKSE/Plugins/Shared.ini")), "mod's");
        assert!(state_deployed(&root));

        // The player edits the settings file; v2 changes the dll and drops a file.
        std::fs::write(data.join("SKSE/Plugins/SkyCraft.ini"), "player's").unwrap();
        let v2 = t.path().join("root/Apps/skycraft/v2");
        t.write("root/Apps/skycraft/v2/SKSE/Plugins/SkyCraft.dll", b"dll v2");
        t.write("root/Apps/skycraft/v2/SKSE/Plugins/SkyCraft.ini", b"ini v2");
        t.write("root/Apps/skycraft/v2/SKSE/Plugins/Shared.ini", b"mod's");
        deploy(&root, "skycraft", &v2, &data, &|_| {}).unwrap();
        assert_eq!(read(&data.join("SKSE/Plugins/SkyCraft.dll")), "dll v2");
        assert_eq!(read(&data.join("SKSE/Plugins/SkyCraft.ini")), "player's");
        assert!(!data.join("SKSE/Plugins/SkyCraft/Bundle/old.txt").exists());
        assert!(!data.join("SKSE/Plugins/SkyCraft").exists(), "emptied folders it made are removed");

        // Removing puts the game back exactly: its own file restored, everything else gone.
        remove(&root, "skycraft").unwrap();
        assert_eq!(read(&data.join("SKSE/Plugins/Shared.ini")), "game's own");
        assert_eq!(read(&data.join("Skyrim.esm")), "game");
        assert!(!data.join("SKSE/Plugins/SkyCraft.dll").exists());
        assert!(!data.join("SKSE/Plugins/SkyCraft.ini").exists());
        assert!(data.join("SKSE/Plugins").is_dir(), "folders the game had stay");
        assert!(!root.apps().join("skycraft").exists());
    }

    fn state_deployed(root: &DataRoot) -> bool {
        manifest_path(root, "skycraft").is_file()
    }

    #[test]
    fn needs_and_plans() {
        let t = TempDir::new("mashup-plan");
        t.file("Game/SkyrimSE.exe", 10);
        let game = t.path().join("Game");
        let host = HostDef { id: "skyrim-se".into(), name: "Skyrim Special Edition".into(), steam: None, exe: "SkyrimSE.exe".into(), dir: Some(path_str(&game)) };
        let mut cat = Catalog::load(None).unwrap();
        cat.hosts = vec![host];
        let def = cat.mashup("skycraft").unwrap().clone();
        let err = plan(&cat, &def, "", None).unwrap_err().to_string();
        assert!(err.contains("SKSE64") && err.contains("Address Library"), "{err}");

        t.file("Game/skse64_loader.exe", 10);
        t.file("Game/Data/SKSE/Plugins/versionlib-1-6-1170-0.bin", 10);
        let p = plan(&cat, &def, "", None).unwrap();
        assert_eq!(p.program, game.join("skse64_loader.exe"));
        assert_eq!(p.cwd, game);
    }
}
