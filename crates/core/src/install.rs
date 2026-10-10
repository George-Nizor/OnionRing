//! Installs ports and emulators into the data root.
//!
//! Layout (like Scoop, so updates never clobber saves):
//! ```text
//! Apps\<id>\<version>\     unpacked release, one folder per version (last two kept)
//! Apps\<id>\current        junction -> the active version (stable path for shortcuts)
//! Apps\<id>\persist\<dir>  saves/settings, junctioned into every version
//! Apps\<id>\app.json       what is installed, from where, and its verified hash
//! Downloads\<id>\<tag>\    the original archive, kept for offline reinstall/rollback
//! ```

use crate::catalog::Catalog;
use crate::github::{self, Release};
use crate::paths::{DataRoot, same_volume};
use crate::util::{self, now, sha256_file};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppSource {
    pub repo: Option<String>,
    pub tag: Option<String>,
    pub asset: Option<String>,
    pub sha256: String,
    /// The archive's hash matched the one GitHub published for that release.
    pub verified: bool,
    pub archive: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    /// "port" or "emulator"
    pub kind: String,
    pub version: String,
    pub installed_at: i64,
    /// Relative to the version folder.
    pub exe: String,
    #[serde(default)]
    pub configure_exe: Option<String>,
    pub size: u64,
    pub source: AppSource,
    #[serde(default)]
    pub versions: Vec<String>,
}

impl InstalledApp {
    pub fn dir(&self, root: &DataRoot) -> PathBuf {
        root.apps().join(&self.id).join("current")
    }
    pub fn exe_path(&self, root: &DataRoot) -> Option<PathBuf> {
        let p = self.dir(root).join(&self.exe);
        p.is_file().then_some(p)
    }
    pub fn configure_path(&self, root: &DataRoot) -> Option<PathBuf> {
        let p = self.dir(root).join(self.configure_exe.as_ref()?);
        p.is_file().then_some(p)
    }
}

pub fn installed_apps(root: &DataRoot) -> Vec<InstalledApp> {
    let Ok(rd) = std::fs::read_dir(root.apps()) else { return vec![] };
    rd.filter_map(|e| e.ok())
        .filter_map(|e| std::fs::read_to_string(e.path().join("app.json")).ok())
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect()
}

pub fn installed(root: &DataRoot, id: &str) -> Option<InstalledApp> {
    let s = std::fs::read_to_string(root.apps().join(id).join("app.json")).ok()?;
    serde_json::from_str(&s).ok()
}

/// What to install: a port or an emulator from the catalog, flattened to what the installer needs.
#[derive(Clone, Debug)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub repo: Option<String>,
    pub asset: Option<String>,
    pub prerelease: bool,
    pub exe: Option<String>,
    pub exe_regex: Option<String>,
    pub configure_exe: Option<String>,
    pub persist: Vec<String>,
    pub carry: Vec<String>,
    pub portable_marker: Option<String>,
}

impl Target {
    pub fn from_catalog(cat: &Catalog, id: &str) -> Result<Target> {
        if let Some(p) = cat.port(id) {
            return Ok(Target {
                id: p.id.clone(),
                name: p.name.clone(),
                kind: "port",
                repo: Some(p.repo.clone()),
                asset: Some(p.asset.clone()),
                prerelease: p.prerelease,
                exe: p.exe.clone(),
                exe_regex: None,
                configure_exe: p.configure_exe.clone(),
                persist: p.persist.clone(),
                carry: p.carry.clone(),
                portable_marker: None,
            });
        }
        if let Some(e) = cat.emulator(id) {
            return Ok(Target {
                id: e.id.clone(),
                name: e.name.clone(),
                kind: "emulator",
                repo: e.repo.clone(),
                asset: e.asset.clone(),
                prerelease: e.prerelease,
                exe: None,
                exe_regex: Some(e.exe.clone()),
                configure_exe: None,
                persist: e.persist.clone(),
                carry: e.carry.clone(),
                portable_marker: e.portable.clone(),
            });
        }
        bail!("{id} is not in the catalog")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArchiveMode {
    /// Move the archive into Downloads (what the user asked for when adopting a manual download).
    Move,
    Copy,
}

pub fn system_tar() -> PathBuf {
    let sys = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    sys.join("System32").join("tar.exe")
}

/// Unpack zip/7z/tar archives with Windows' built-in bsdtar (libarchive). It refuses absolute
/// and `..` paths, so archives cannot write outside `dest`.
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let mut cmd = Command::new(system_tar());
    cmd.arg("-xf").arg(archive).arg("-C").arg(dest);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().context("running tar.exe")?;
    if !out.status.success() {
        bail!("could not unpack {}: {}", archive.display(), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn safe_version(v: &str) -> String {
    v.chars().map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect()
}

fn remove_link(p: &Path) {
    if std::fs::symlink_metadata(p).is_ok() {
        let _ = junction::delete(p);
        let _ = std::fs::remove_dir(p);
    }
}

fn is_link(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink() || junction::exists(p).unwrap_or(false)).unwrap_or(false)
}

/// Find which published release an archive came from by its SHA-256.
pub fn identify(root: &DataRoot, repo: &str, sha: &str) -> Option<(Release, String)> {
    let rels = github::releases(root, repo, 30).ok()?;
    for r in rels {
        if let Some(a) = r.assets.iter().find(|a| r.sha256_of(a).as_deref() == Some(sha)) {
            let name = a.name.clone();
            return Some((r, name));
        }
    }
    None
}

fn detect_exe(dir: &Path, t: &Target) -> Option<String> {
    if let Some(e) = &t.exe
        && dir.join(e).is_file() {
            return Some(e.clone());
        }
    let skip = regex::Regex::new(r"(?i)unins|setup|install|update|crash|redist|dxsetup|probe|capabilit|helper|tool|room|server|debug").unwrap();
    let re = t.exe_regex.as_deref().and_then(|r| regex::RegexBuilder::new(r).case_insensitive(true).build().ok());
    let mut cands: Vec<(usize, String)> = walkdir::WalkDir::new(dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().ends_with(".exe") {
                return None;
            }
            let rel = e.path().strip_prefix(dir).ok()?.to_string_lossy().to_string();
            match &re {
                Some(r) => r.is_match(&name).then_some((e.depth(), rel)),
                None => (!skip.is_match(&name)).then_some((e.depth(), rel)),
            }
        })
        .collect();
    cands.sort();
    cands.into_iter().next().map(|(_, r)| r)
}

/// Install from an archive already on disk (a manual download, or one we just fetched).
pub fn install_archive(
    root: &DataRoot,
    t: &Target,
    archive: &Path,
    mode: ArchiveMode,
    known: Option<(Release, String)>,
    progress: &dyn Fn(&str),
) -> Result<InstalledApp> {
    if !archive.is_file() {
        bail!("{} does not exist", archive.display());
    }
    progress("Checking archive");
    let sha = sha256_file(archive)?;
    let known = known.or_else(|| t.repo.as_deref().and_then(|r| identify(root, r, &sha)));
    let (version, tag, asset, verified) = match &known {
        Some((rel, asset)) => {
            let published = rel.assets.iter().find(|a| &a.name == asset).and_then(|a| rel.sha256_of(a));
            if let Some(p) = &published
                && p != &sha {
                    bail!("{} does not match the hash GitHub published for {} ({}); refusing to install", asset, rel.tag_name, p);
                }
            (rel.tag_name.clone(), Some(rel.tag_name.clone()), Some(asset.clone()), published.is_some())
        }
        None => (format!("local-{}", now()), None, None, false),
    };
    let vname = safe_version(&version);
    let app_dir = root.apps().join(&t.id);
    std::fs::create_dir_all(&app_dir)?;

    // Keep the archive under Downloads.
    let file_name = archive.file_name().context("archive has no name")?;
    let kept = root.downloads().join(&t.id).join(&vname).join(file_name);
    if kept != archive {
        std::fs::create_dir_all(kept.parent().unwrap())?;
        match mode {
            ArchiveMode::Move if same_volume(archive, &kept) => std::fs::rename(archive, &kept)?,
            ArchiveMode::Move => {
                std::fs::copy(archive, &kept)?;
                std::fs::remove_file(archive)?;
            }
            ArchiveMode::Copy => {
                std::fs::copy(archive, &kept)?;
            }
        }
    }

    progress("Unpacking");
    let staging = app_dir.join(".staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    extract(&kept, &staging)?;
    let entries: Vec<_> = std::fs::read_dir(&staging)?.filter_map(|e| e.ok()).collect();
    let content = if entries.len() == 1 && entries[0].path().is_dir() { entries[0].path() } else { staging.clone() };

    let vdir = app_dir.join(&vname);
    if vdir.exists() {
        unlink_persist(&vdir, &t.persist);
        std::fs::remove_dir_all(&vdir)?;
    }
    std::fs::rename(&content, &vdir).with_context(|| format!("moving into {}", vdir.display()))?;
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }

    progress("Keeping your saves and settings");
    let previous = installed(root, &t.id);
    let persist_root = app_dir.join("persist");
    for p in &t.persist {
        let in_version = vdir.join(p);
        let kept_dir = persist_root.join(p);
        if in_version.exists() && !is_link(&in_version) {
            if kept_dir.exists() {
                std::fs::remove_dir_all(&in_version)?;
            } else {
                std::fs::create_dir_all(kept_dir.parent().unwrap())?;
                std::fs::rename(&in_version, &kept_dir)?;
            }
        }
        std::fs::create_dir_all(&kept_dir)?;
        if let Some(parent) = in_version.parent() {
            std::fs::create_dir_all(parent)?;
        }
        junction::create(&kept_dir, &in_version).with_context(|| format!("linking {}", in_version.display()))?;
    }
    if let Some(prev) = previous.as_ref().filter(|p| p.version != version) {
        let prev_dir = app_dir.join(safe_version(&prev.version));
        for f in &t.carry {
            let (from, to) = (prev_dir.join(f), vdir.join(f));
            if from.is_file() {
                let _ = std::fs::copy(&from, &to);
            }
        }
    }
    if let Some(marker) = &t.portable_marker {
        let m = vdir.join(marker);
        if !m.exists() {
            std::fs::write(m, "")?;
        }
    }

    let current = app_dir.join("current");
    remove_link(&current);
    junction::create(&vdir, &current).context("pointing current at the new version")?;

    let exe = detect_exe(&vdir, t).context("could not find the program to start in this download")?;
    let mut versions: Vec<String> = previous.as_ref().map(|p| p.versions.clone()).unwrap_or_default();
    versions.retain(|v| v != &version);
    versions.insert(0, version.clone());
    // Keep the active and one previous version for rollback.
    for old in versions.split_off(2.min(versions.len())) {
        let d = app_dir.join(safe_version(&old));
        unlink_persist(&d, &t.persist);
        let _ = std::fs::remove_dir_all(&d);
    }

    let app = InstalledApp {
        id: t.id.clone(),
        name: t.name.clone(),
        kind: t.kind.to_string(),
        version: version.clone(),
        installed_at: now(),
        configure_exe: t.configure_exe.clone().filter(|c| vdir.join(c).is_file()),
        exe,
        size: util::dir_size(&vdir),
        source: AppSource {
            repo: t.repo.clone(),
            tag,
            asset,
            sha256: sha,
            verified,
            archive: util::path_str(&kept),
        },
        versions,
    };
    std::fs::write(app_dir.join("app.json"), serde_json::to_string_pretty(&app)?)?;
    progress("Installed");
    Ok(app)
}

/// Remove persist junctions from a version folder before deleting it, so deletion can never
/// reach the saves they point at.
fn unlink_persist(vdir: &Path, persist: &[String]) {
    for p in persist {
        let link = vdir.join(p);
        if is_link(&link) {
            remove_link(&link);
        }
    }
}

/// Download the latest release from GitHub and install it.
pub fn install_latest(root: &DataRoot, t: &Target, progress: &dyn Fn(&str)) -> Result<InstalledApp> {
    let repo = t.repo.as_deref().context("this entry has no GitHub releases; download it from its homepage")?;
    let pattern = t.asset.as_deref().context("no Windows download pattern for this entry")?;
    progress("Looking up the latest release");
    let rel = github::latest(root, repo, t.prerelease)?;
    let asset = github::pick_asset(&rel, pattern)?.clone();
    let dest = root.downloads().join(&t.id).join(safe_version(&rel.tag_name)).join(&asset.name);
    let expected = rel.sha256_of(&asset);
    let have = dest.is_file() && expected.as_deref().is_some_and(|e| sha256_file(&dest).ok().as_deref() == Some(e));
    if !have {
        github::download(&asset.browser_download_url, &dest, &|done, total| {
            let pct = if total > 0 { done * 100 / total } else { 0 };
            progress(&format!("Downloading {} ({pct}%)", asset.name));
        })?;
    }
    let name = asset.name.clone();
    install_archive(root, t, &dest, ArchiveMode::Copy, Some((rel, name)), progress)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub id: String,
    pub installed: String,
    pub latest: String,
    pub published: Option<String>,
    pub available: bool,
    pub url: Option<String>,
}

pub fn check_update(root: &DataRoot, t: &Target) -> Result<Option<UpdateInfo>> {
    let Some(app) = installed(root, &t.id) else { return Ok(None) };
    let Some(repo) = t.repo.as_deref() else { return Ok(None) };
    let rel = github::latest(root, repo, t.prerelease)?;
    Ok(Some(UpdateInfo {
        id: t.id.clone(),
        available: app.source.tag.as_deref() != Some(rel.tag_name.as_str()),
        installed: app.version,
        latest: rel.tag_name,
        published: rel.published_at,
        url: rel.html_url,
    }))
}
