//! Built-in knowledge: platforms, emulators and installable ports.
//!
//! Definitions ship inside the binary (`data/*.json`). A user can add or override entries with
//! `<data root>\catalog.local.json` (same shape, merged by `id`) without waiting for a release.

use anyhow::Result;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub short: String,
    pub color: String,
    #[serde(default)]
    pub libretro: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub hints: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDef {
    pub id: String,
    pub name: String,
    pub platforms: Vec<String>,
    /// Case-insensitive regex for the executable's file name.
    pub exe: String,
    /// One argv element per entry. Placeholders: `{rom}`, `{rom_dir}`, `{emu_dir}`.
    pub args: Vec<String>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    /// Marker file that switches the emulator to portable mode (created on install).
    #[serde(default)]
    pub portable: Option<String>,
    /// Folders kept across updates when installed by OnionRing (portable data).
    #[serde(default)]
    pub persist: Vec<String>,
    /// Files copied forward from the previous version on update.
    #[serde(default)]
    pub carry: Vec<String>,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PortDef {
    pub id: String,
    /// The game this port plays, e.g. "Bloodborne".
    pub game: String,
    /// The project, e.g. "bbport for Windows".
    pub name: String,
    /// "Native port", "Recomp", "Decomp", "Engine recreation".
    pub kind: String,
    #[serde(default)]
    pub original: Vec<String>,
    pub repo: String,
    /// Regex picking the Windows asset from a GitHub release.
    pub asset: String,
    #[serde(default)]
    pub prerelease: bool,
    /// Exe (relative to the install folder) that starts the game. Detected when absent.
    #[serde(default)]
    pub exe: Option<String>,
    /// Optional second exe for settings/configuration.
    #[serde(default)]
    pub configure_exe: Option<String>,
    /// Folders (relative) kept across updates: they live in `persist\` and are junctioned in.
    #[serde(default)]
    pub persist: Vec<String>,
    /// Files (relative) copied forward from the previous version on update.
    #[serde(default)]
    pub carry: Vec<String>,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub licence: Option<String>,
    #[serde(default)]
    pub blurb: Option<String>,
    /// Relative path inside the install to use as cover/hero art.
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Deserialize, Default)]
struct LocalCatalog {
    #[serde(default)]
    platforms: Vec<Platform>,
    #[serde(default)]
    emulators: Vec<EmulatorDef>,
    #[serde(default)]
    ports: Vec<PortDef>,
}

pub struct Catalog {
    pub platforms: Vec<Platform>,
    pub emulators: Vec<EmulatorDef>,
    pub ports: Vec<PortDef>,
    pub(crate) hint_res: Vec<Option<Regex>>,
    pub(crate) exe_res: Vec<Regex>,
}

fn merge<T: Clone>(base: &mut Vec<T>, extra: Vec<T>, id: impl Fn(&T) -> &str) {
    for e in extra {
        if let Some(slot) = base.iter_mut().find(|b| id(b) == id(&e)) {
            *slot = e;
        } else {
            base.push(e);
        }
    }
}

fn ci(re: &str) -> Result<Regex> {
    Ok(RegexBuilder::new(re).case_insensitive(true).build()?)
}

impl Catalog {
    pub fn load(local: Option<&Path>) -> Result<Self> {
        let mut platforms: Vec<Platform> = serde_json::from_str(include_str!("../data/platforms.json"))?;
        let mut emulators: Vec<EmulatorDef> = serde_json::from_str(include_str!("../data/emulators.json"))?;
        let mut ports: Vec<PortDef> = serde_json::from_str(include_str!("../data/ports.json"))?;
        if let Some(p) = local.filter(|p| p.exists()) {
            match std::fs::read_to_string(p).map_err(anyhow::Error::from).and_then(|s| Ok(serde_json::from_str::<LocalCatalog>(&s)?)) {
                Ok(l) => {
                    merge(&mut platforms, l.platforms, |x| &x.id);
                    merge(&mut emulators, l.emulators, |x| &x.id);
                    merge(&mut ports, l.ports, |x| &x.id);
                }
                Err(e) => eprintln!("ignoring {}: {e}", p.display()),
            }
        }
        let hint_res = platforms
            .iter()
            .map(|p| if p.hints.is_empty() { Ok(None) } else { ci(&p.hints).map(Some) })
            .collect::<Result<Vec<_>>>()?;
        let exe_res = emulators.iter().map(|e| ci(&e.exe)).collect::<Result<Vec<_>>>()?;
        Ok(Catalog { platforms, emulators, ports, hint_res, exe_res })
    }

    pub fn platform(&self, id: &str) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.id == id)
    }
    pub fn emulator(&self, id: &str) -> Option<&EmulatorDef> {
        self.emulators.iter().find(|e| e.id == id)
    }
    pub fn port(&self, id: &str) -> Option<&PortDef> {
        self.ports.iter().find(|p| p.id == id)
    }

    /// Emulator definition whose exe pattern matches this file name.
    pub fn match_emulator_exe(&self, file_name: &str) -> Option<&EmulatorDef> {
        self.exe_res.iter().position(|r| r.is_match(file_name)).map(|i| &self.emulators[i])
    }

    /// Platforms that list this extension.
    pub fn platforms_for_ext(&self, ext: &str) -> Vec<&str> {
        self.platforms
            .iter()
            .filter(|p| p.extensions.iter().any(|e| e == ext))
            .map(|p| p.id.as_str())
            .collect()
    }

    pub fn hint_matches(&self, platform: &str, segment: &str) -> bool {
        self.platforms
            .iter()
            .position(|p| p.id == platform)
            .and_then(|i| self.hint_res[i].as_ref())
            .is_some_and(|r| r.is_match(segment))
    }

    pub fn is_rom_ext(&self, ext: &str) -> bool {
        !ext.is_empty() && self.platforms.iter().any(|p| p.extensions.iter().any(|e| e == ext))
    }
}
