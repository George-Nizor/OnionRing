//! Update checks for everything that runs games.
//!
//! - Apps OnionRing installed are compared by release tag (`install::check_update`) and can be
//!   updated in place.
//! - Emulators found on the PC carry the version from their folder name (`pcsx2-v2.2.0-...`).
//!   That is compared numerically with the newest GitHub release; the player updates those
//!   themselves, so the result links to the release.

use crate::catalog::{Catalog, EmulatorDef};
use crate::db::Db;
use crate::github::{self, Release};
use crate::install::{self, Target};
use crate::paths::DataRoot;
use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)+)(?:-(\d{3,}))?").unwrap());

/// A version number as projects write it: "v2.2.0", "0.0.37-18146" (RPCS3 adds a build
/// number), "2122.1" (Azahar). Only the first version-looking part of a string is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub parts: Vec<u64>,
    /// A build counter after the version ("-18146"). Only compared when both sides have one.
    pub build: Option<u64>,
}

impl Version {
    /// `None` when there is no dotted number in `s` (tags like "latest" or "build-0fd6956b").
    pub fn parse(s: &str) -> Option<Version> {
        let c = VERSION.captures(s)?;
        let parts = c[1].split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>()?;
        Some(Version { parts, build: c.get(2).and_then(|b| b.as_str().parse().ok()) })
    }

    /// Missing trailing parts count as zero (2.2 == 2.2.0). Build numbers break ties only when
    /// both versions have one.
    pub fn compare(&self, other: &Version) -> Ordering {
        let n = self.parts.len().max(other.parts.len());
        for i in 0..n {
            let (a, b) = (self.parts.get(i).copied().unwrap_or(0), other.parts.get(i).copied().unwrap_or(0));
            if a != b {
                return a.cmp(&b);
            }
        }
        match (self.build, other.build) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => Ordering::Equal,
        }
    }

    pub fn is_newer_than(&self, other: &Version) -> bool {
        self.compare(other) == Ordering::Greater
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.parts.iter().map(|p| p.to_string()).collect();
        write!(f, "{}", parts.join("."))?;
        if let Some(b) = self.build {
            write!(f, "-{b}")?;
        }
        Ok(())
    }
}

/// The version a release ships: from its tag, else its name, else the download's file name
/// (RPCS3 tags are "build-<hash>" but its downloads are named "rpcs3-v0.0.37-18146-...").
/// A build number found only in the file name is added when the versions agree.
pub fn release_version(rel: &Release, asset: Option<&str>) -> Option<Version> {
    let found: Vec<Version> = [Some(rel.tag_name.as_str()), rel.name.as_deref(), asset].into_iter().flatten().filter_map(Version::parse).collect();
    let mut v = found.first()?.clone();
    if v.build.is_none() {
        v.build = found.iter().find(|o| o.build.is_some() && o.compare(&v) == Ordering::Equal).and_then(|o| o.build);
    }
    Some(v)
}

/// The newest release of an emulator, as far as an update check needs it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LatestRelease {
    /// The readable version ("0.0.38-18212"), or the tag when no version can be read from it.
    pub label: String,
    #[serde(skip)]
    pub version: Option<Version>,
    pub published: Option<String>,
    pub url: Option<String>,
}

/// Look up the newest release of an emulator's project. `None` when it has no GitHub releases.
pub fn latest_release(root: &DataRoot, def: &EmulatorDef) -> Result<Option<LatestRelease>> {
    let Some(repo) = def.repo.as_deref() else { return Ok(None) };
    let rel = github::latest(root, repo, def.prerelease)?;
    let asset = def.asset.as_deref().and_then(|p| github::pick_asset(&rel, p).ok()).map(|a| a.name.clone());
    let version = release_version(&rel, asset.as_deref());
    Ok(Some(LatestRelease {
        label: version.as_ref().map(|v| v.to_string()).unwrap_or_else(|| rel.tag_name.clone()),
        version,
        published: rel.published_at.clone(),
        url: rel.html_url.clone().or_else(|| def.homepage.clone()),
    }))
}

/// Whether an emulator found on the PC is behind the newest release.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoundCheck {
    pub installed: String,
    pub latest: String,
    /// `None` when either version cannot be read, so nothing can be said.
    pub newer: Option<bool>,
    pub published: Option<String>,
    pub url: Option<String>,
}

/// Compare a found emulator's version (from its folder name) with a release.
pub fn compare_found(installed: &str, latest: &LatestRelease) -> FoundCheck {
    let newer = match (Version::parse(installed), latest.version.as_ref()) {
        (Some(have), Some(new)) => Some(new.is_newer_than(&have)),
        _ => None,
    };
    FoundCheck {
        installed: installed.to_string(),
        latest: latest.label.clone(),
        newer,
        published: latest.published.clone(),
        url: latest.url.clone(),
    }
}

/// Check one emulator found on the PC against its newest release. `None` when the project has
/// no GitHub releases to compare with.
pub fn check_found_emulator(root: &DataRoot, def: &EmulatorDef, installed_version: &str) -> Result<Option<FoundCheck>> {
    Ok(latest_release(root, def)?.map(|l| compare_found(installed_version, &l)))
}

/// An update that can be installed.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    /// App id for apps OnionRing installed, emulator id (as in `launch::emulators`) otherwise.
    pub id: String,
    pub name: String,
    /// "port" or "emulator".
    pub kind: String,
    pub installed: String,
    pub latest: String,
    /// OnionRing installed it and can update it. Otherwise it was found on the PC and the
    /// player updates it from `url`.
    pub managed: bool,
    pub published: Option<String>,
    pub url: Option<String>,
}

/// Every update available: apps OnionRing installed plus emulators found on the PC whose
/// version can be read from their folder name. Lookups run in parallel and use the GitHub
/// cache. Fails only when every lookup failed (offline, rate limited).
pub fn available_updates(root: &DataRoot, cat: &Catalog, db: &Db) -> Result<Vec<AvailableUpdate>> {
    // Emulators found on the PC (OnionRing's own are covered by the installed apps below).
    let found: Vec<(String, String, String, &EmulatorDef)> = db
        .emulator_rows()?
        .into_iter()
        .filter(|(_, _, exe, _, _, managed, _)| !managed && Path::new(exe).is_file())
        .filter_map(|(id, name, _, version, _, _, _)| {
            let def = cat.emulator_for_instance(&id).filter(|d| d.repo.is_some())?;
            Some((id, name, version?, def))
        })
        .collect();
    let mut defs: Vec<&EmulatorDef> = found.iter().map(|f| f.3).collect();
    defs.sort_by(|a, b| a.id.cmp(&b.id));
    defs.dedup_by(|a, b| a.id == b.id);
    let latest: HashMap<&str, Result<Option<LatestRelease>>> =
        defs.par_iter().map(|d| (d.id.as_str(), latest_release(root, d))).collect();

    let apps = install::installed_apps(root);
    let managed: Vec<(install::InstalledApp, Result<Option<install::UpdateInfo>>)> = apps
        .into_par_iter()
        .filter_map(|app| {
            let t = Target::from_catalog(cat, &app.id).ok()?;
            let r = install::check_update(root, &t);
            Some((app, r))
        })
        .collect();

    let mut out = Vec::new();
    let mut ok = 0;
    let mut first_err = None;
    for (app, r) in managed {
        match r {
            Ok(u) => {
                ok += 1;
                if let Some(u) = u.filter(|u| u.available) {
                    out.push(AvailableUpdate {
                        id: app.id,
                        name: app.name,
                        kind: app.kind,
                        installed: u.installed,
                        latest: u.latest,
                        managed: true,
                        published: u.published,
                        url: u.url,
                    });
                }
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    ok += latest.values().filter(|r| r.is_ok()).count();
    for (id, name, version, def) in found {
        let Some(Ok(Some(l))) = latest.get(def.id.as_str()) else { continue };
        let c = compare_found(&version, l);
        if c.newer == Some(true) {
            out.push(AvailableUpdate {
                id,
                name,
                kind: "emulator".into(),
                installed: c.installed,
                latest: c.latest,
                managed: false,
                published: c.published,
                url: c.url,
            });
        }
    }
    if ok == 0 {
        let err = latest.into_values().find_map(|r| r.err()).or(first_err);
        if let Some(e) = err {
            return Err(e.context("Could not check for updates."));
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("no version in {s}"))
    }

    fn rel(tag: &str, name: Option<&str>) -> Release {
        Release { tag_name: tag.into(), name: name.map(String::from), prerelease: false, published_at: None, html_url: None, assets: vec![] }
    }

    #[test]
    fn reads_versions() {
        assert_eq!(v("v2.2.0"), Version { parts: vec![2, 2, 0], build: None });
        assert_eq!(v("pcsx2-v2.3.70-windows-x64-Qt"), Version { parts: vec![2, 3, 70], build: None });
        assert_eq!(v("rpcs3-v0.0.37-18146-6f5d6b7e_win64_msvc.7z"), Version { parts: vec![0, 0, 37], build: Some(18146) });
        assert_eq!(v("0.0.34-17210"), Version { parts: vec![0, 0, 34], build: Some(17210) });
        assert_eq!(v("v.0.9.0"), Version { parts: vec![0, 9, 0], build: None });
        assert_eq!(v("azahar-2122.1-windows-msvc.zip"), Version { parts: vec![2122, 1], build: None });
        assert_eq!(v("desmume-0.9.13-win64"), Version { parts: vec![0, 9, 13], build: None });
        for none in ["latest", "build-0fd6956b29a2", "duckstation-windows-x64-release.zip", "xemu-win-x86_64-release.zip", "Dolphin-x64"] {
            assert_eq!(Version::parse(none), None, "{none}");
        }
        assert_eq!(v("0.0.37-18146").to_string(), "0.0.37-18146");
        assert_eq!(v("v2.2").to_string(), "2.2");
    }

    #[test]
    fn compares_numerically() {
        assert!(v("2.10.0").is_newer_than(&v("2.9.9")));
        assert!(v("2.2.1").is_newer_than(&v("2.2")));
        assert!(!v("2.2.0").is_newer_than(&v("2.2")));
        assert!(!v("2.2").is_newer_than(&v("2.2.0")));
        assert!(v("0.0.38-18212").is_newer_than(&v("0.0.37-18146")));
        assert!(v("0.0.37-18200").is_newer_than(&v("0.0.37-18146")));
        assert!(!v("0.0.37-18146").is_newer_than(&v("0.0.37-18200")));
        // Without a build number on one side, the same version is not an update.
        assert!(!v("0.0.37-18200").is_newer_than(&v("0.0.37")));
        assert!(!v("1.0").is_newer_than(&v("1.0.0-12345")));
        assert!(v("2123.0").is_newer_than(&v("2122.1")));
    }

    #[test]
    fn release_versions() {
        // RPCS3: nothing in the tag, everything in the download's name.
        let r = rel("build-6f5d6b7e0c", Some("build-6f5d6b7e0c"));
        assert_eq!(release_version(&r, Some("rpcs3-v0.0.38-18212-6f5d6b7e_win64_msvc.7z")), Some(v("0.0.38-18212")));
        // The tag wins; a matching build number from the file name is kept.
        let r = rel("v0.0.38", None);
        assert_eq!(release_version(&r, Some("rpcs3-v0.0.38-18212_win64_msvc.7z")), Some(v("0.0.38-18212")));
        assert_eq!(release_version(&r, Some("thing-v0.0.39-18300.7z")), Some(Version { parts: vec![0, 0, 38], build: None }));
        // Rolling releases with no version anywhere.
        assert_eq!(release_version(&rel("latest", Some("Latest Rolling Release")), Some("duckstation-windows-x64-release.zip")), None);
        assert_eq!(release_version(&rel("latest", None), None), None);
    }

    #[test]
    fn found_emulators() {
        let latest = LatestRelease { label: "0.0.38-18212".into(), version: Some(v("0.0.38-18212")), published: None, url: Some("u".into()) };
        let c = compare_found("0.0.34-17210", &latest);
        assert_eq!((c.newer, c.latest.as_str(), c.installed.as_str()), (Some(true), "0.0.38-18212", "0.0.34-17210"));
        assert_eq!(compare_found("0.0.38-18212", &latest).newer, Some(false));
        assert_eq!(compare_found("0.0.38", &latest).newer, Some(false));
        assert_eq!(compare_found("RPCS3", &latest).newer, None);
        let unknown = LatestRelease { label: "latest".into(), version: None, published: None, url: None };
        assert_eq!(compare_found("0.1-9226", &unknown).newer, None);
    }
}
