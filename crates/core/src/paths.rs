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
