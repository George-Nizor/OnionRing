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
    /// Files only the player can supply (BIOS, firmware, keys). See `checks::run_checks`.
    #[serde(default)]
    pub checks: Vec<CheckDef>,
    /// Folders that hold saves, as path templates (see `paths::expand`). Used by backups.
    #[serde(default)]
    pub saves: Vec<String>,
    /// Folder names left out of save backups, e.g. installed game data that sits inside a
    /// save tree ("content" in a 3DS or Wii NAND).
    #[serde(default)]
    pub saves_skip: Vec<String>,
}

/// One file an emulator needs that only the player can supply.
///
/// The check passes when any of `paths` (path templates, `*` allowed in the last segment) is a
/// file of at least `min_size` bytes, or when one of `settings` points at such a file.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CheckDef {
    /// What the file is, in plain words ("PS2 BIOS").
    pub what: String,
    #[serde(default)]
    pub paths: Vec<String>,
    /// Emulator settings that name where the file (or its folder) is.
    #[serde(default)]
    pub settings: Vec<SettingRef>,
    /// Smallest size a real copy can have, so empty or placeholder files do not count.
    #[serde(default)]
    pub min_size: u64,
    /// For text files: a regex at least one line must match (e.g. a key).
    #[serde(default)]
    pub contains: Option<String>,
    /// Lines matching this regex never count towards `contains` (e.g. an example key).
    #[serde(default)]
    pub ignore: Option<String>,
    /// The emulator works without it (it only improves accuracy or unlocks extras).
    #[serde(default)]
    pub optional: bool,
    /// One plain sentence on how to get it from your own console.
    #[serde(default)]
    pub help: String,
}

/// A setting in an emulator's INI/TOML config that names a file or a folder.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingRef {
    /// The config file (path template).
    pub file: String,
    /// Only look inside this `[section]` (INI section or TOML table). Any section when absent.
    #[serde(default)]
    pub section: Option<String>,
    pub key: String,
    /// Folder that relative values are resolved against, relative to the config file's folder
    /// ("." when absent; ".." when the config lives in a subfolder of the data folder).
    #[serde(default)]
    pub base: Option<String>,
    /// The setting names a folder; look for files matching this pattern inside it.
    #[serde(default)]
    pub glob: Option<String>,
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

/// A game that runs in a web page (WebAssembly/WebGL). Only legitimate builds are listed:
/// official, open-source, shareware/demo data, fan-made originals, or "bring your own files".
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrowserGame {
    pub id: String,
    pub title: String,
    pub project: String,
    pub url: String,
    #[serde(default)]
    pub source: Option<String>,
    /// official | open-source | shareware | own-files | fan-original
    pub status: String,
    #[serde(default)]
    pub original: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub needs_files: Option<String>,
    /// "window": open in an OnionRing window; "link": open in the user's browser.
    pub embed: String,
    #[serde(default)]
    pub controls: Option<String>,
    #[serde(default)]
    pub multiplayer: bool,
    #[serde(default)]
    pub verified: Option<String>,
}

fn read_local(path: &Path) -> Result<serde_json::Value> {
    Ok(match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s)?,
        Err(_) => serde_json::json!({}),
    })
}

/// Add a browser game of the user's own to `catalog.local.json` (other keys are kept).
pub fn add_local_browser_game(path: &Path, title: &str, url: &str) -> Result<BrowserGame> {
    let parsed = url::Url::parse(url.trim()).map_err(|_| anyhow::anyhow!("That doesn't look like a web address."))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!("Only http and https links can be added.");
    }
    let title = title.trim();
    if title.is_empty() {
        anyhow::bail!("Give it a name.");
    }
    let slug: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let game = BrowserGame {
        id: format!("my-{}-{}", slug.trim_matches('-'), crate::util::now()),
        title: title.to_string(),
        project: parsed.host_str().unwrap_or_default().to_string(),
        url: parsed.to_string(),
        source: None,
        status: "custom".into(),
        original: None,
        summary: None,
        needs_files: None,
        embed: "window".into(),
        controls: None,
        multiplayer: false,
        verified: None,
    };
    let mut v = read_local(path)?;
    let list = v.as_object_mut().ok_or_else(|| anyhow::anyhow!("catalog.local.json is not an object"))?.entry("browser").or_insert(serde_json::json!([]));
    list.as_array_mut().ok_or_else(|| anyhow::anyhow!("\"browser\" is not a list"))?.push(serde_json::to_value(&game)?);
    std::fs::write(path, serde_json::to_string_pretty(&v)?)?;
    Ok(game)
}

/// Remove one of the user's own browser games.
pub fn remove_local_browser_game(path: &Path, id: &str) -> Result<()> {
    let mut v = read_local(path)?;
    if let Some(list) = v.get_mut("browser").and_then(|l| l.as_array_mut()) {
        list.retain(|g| g.get("id").and_then(|i| i.as_str()) != Some(id));
    }
    std::fs::write(path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

#[derive(Deserialize, Default)]
struct LocalCatalog {
    #[serde(default)]
    platforms: Vec<Platform>,
    #[serde(default)]
    emulators: Vec<EmulatorDef>,
    #[serde(default)]
    ports: Vec<PortDef>,
    #[serde(default)]
    browser: Vec<BrowserGame>,
}

pub struct Catalog {
    pub platforms: Vec<Platform>,
    pub emulators: Vec<EmulatorDef>,
    pub ports: Vec<PortDef>,
    pub browser: Vec<BrowserGame>,
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
        let mut browser: Vec<BrowserGame> = serde_json::from_str(include_str!("../data/browser-games.json"))?;
        if let Some(p) = local.filter(|p| p.exists()) {
            match std::fs::read_to_string(p).map_err(anyhow::Error::from).and_then(|s| Ok(serde_json::from_str::<LocalCatalog>(&s)?)) {
                Ok(l) => {
                    merge(&mut platforms, l.platforms, |x| &x.id);
                    merge(&mut emulators, l.emulators, |x| &x.id);
                    merge(&mut ports, l.ports, |x| &x.id);
                    merge(&mut browser, l.browser, |x| &x.id);
                }
                Err(e) => eprintln!("ignoring {}: {e}", p.display()),
            }
        }
        let hint_res = platforms
            .iter()
            .map(|p| if p.hints.is_empty() { Ok(None) } else { ci(&p.hints).map(Some) })
            .collect::<Result<Vec<_>>>()?;
        let exe_res = emulators.iter().map(|e| ci(&e.exe)).collect::<Result<Vec<_>>>()?;
        Ok(Catalog { platforms, emulators, ports, browser, hint_res, exe_res })
    }

    pub fn platform(&self, id: &str) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.id == id)
    }
    pub fn emulator(&self, id: &str) -> Option<&EmulatorDef> {
        self.emulators.iter().find(|e| e.id == id)
    }
    /// The definition behind an emulator found on the PC. A second copy of PCSX2 is stored as
    /// `pcsx2-2`, a third as `pcsx2-3`, and so on.
    pub fn emulator_for_instance(&self, instance_id: &str) -> Option<&EmulatorDef> {
        self.emulator(instance_id).or_else(|| {
            let (base, n) = instance_id.rsplit_once('-')?;
            n.parse::<u32>().ok()?;
            self.emulator(base)
        })
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
