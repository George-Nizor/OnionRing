//! Find things to play that the built-in catalog doesn't list: search GitHub for projects that
//! publish Windows builds (ports, recomps, fan games, mods packaged as programs), check what the
//! newest release would download, and add one to `catalog.local.json` as a port so the normal
//! installer (checksums, versioned folders, saves kept across updates) takes it from there.

use crate::catalog::{Catalog, PortDef};
use crate::github::{self, Release};
use crate::paths::DataRoot;
use anyhow::{Context, Result, anyhow, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A GitHub project that matched a search.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// "owner/name".
    pub repo: String,
    pub name: String,
    pub description: Option<String>,
    pub stars: u64,
    /// When the project last changed (ISO 8601).
    pub pushed: Option<String>,
    pub url: String,
    /// The catalog entry for this project, when there already is one.
    pub port_id: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<RepoItem>,
}

#[derive(Deserialize)]
struct RepoItem {
    full_name: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    pushed_at: Option<String>,
    html_url: String,
    #[serde(default)]
    archived: bool,
}

/// Search GitHub for projects matching `query`, most relevant first. Archived projects are left
/// out. Uses the same ETag cache as release lookups, so repeating a search is free.
pub fn search(root: &DataRoot, cat: &Catalog, query: &str) -> Result<Vec<Found>> {
    let q = query.trim();
    if q.chars().count() < 2 {
        bail!("Type at least two letters to search.");
    }
    let url = format!(
        "https://api.github.com/search/repositories?q={}&per_page=30",
        utf8_percent_encode(&format!("{q} fork:false"), NON_ALPHANUMERIC)
    );
    let body = github::get(root, &url)?;
    let resp: SearchResponse = serde_json::from_str(&body).context("GitHub sent a search result OnionRing can't read")?;
    Ok(resp
        .items
        .into_iter()
        .filter(|r| !r.archived)
        .map(|r| Found {
            port_id: cat.ports.iter().find(|p| p.repo.eq_ignore_ascii_case(&r.full_name)).map(|p| p.id.clone()),
            repo: r.full_name,
            name: r.name,
            description: r.description.filter(|d| !d.trim().is_empty()),
            stars: r.stargazers_count,
            pushed: r.pushed_at,
            url: r.html_url,
        })
        .collect())
}

/// What installing a project's newest release would download.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub repo: String,
    /// `None` when the project has published no releases.
    pub tag: Option<String>,
    pub published: Option<String>,
    pub prerelease: bool,
    /// The Windows download OnionRing would pick, or `None` when the release has none.
    pub asset: Option<String>,
    pub size: Option<u64>,
    /// The download is checked against a published SHA-256.
    pub checksum: bool,
}

/// Look at a project's newest release: stable when there is one, else the newest test build.
pub fn inspect(root: &DataRoot, repo: &str) -> Result<Inspection> {
    let repo = valid_repo(repo)?;
    let (rel, prerelease) = newest(root, &repo)?;
    let Some(rel) = rel else {
        return Ok(Inspection { repo, tag: None, published: None, prerelease: false, asset: None, size: None, checksum: false });
    };
    let asset = github::pick_asset(&rel, "auto").ok();
    Ok(Inspection {
        repo,
        tag: Some(rel.tag_name.clone()),
        published: rel.published_at.clone(),
        prerelease,
        asset: asset.map(|a| a.name.clone()),
        size: asset.map(|a| a.size),
        checksum: asset.is_some_and(|a| rel.sha256_of(a).is_some()),
    })
}

/// The release the installer will pick (`github::latest`: the one GitHub marks as latest,
/// else the newest stable, else the newest test build), or `None` when there are none. The
/// flag says whether it is a test build.
fn newest(root: &DataRoot, repo: &str) -> Result<(Option<Release>, bool)> {
    // Asking for the release list first tells a missing project apart from one with no releases.
    let list = github::releases(root, repo, 10).map_err(|e| {
        if format!("{e:#}").contains("not found") {
            anyhow!("There's no GitHub project called {repo}.")
        } else {
            e
        }
    })?;
    if list.is_empty() {
        return Ok((None, false));
    }
    let rel = github::latest(root, repo, false).ok().or_else(|| github::newest_of(list, false));
    let prerelease = rel.as_ref().is_some_and(|r| r.prerelease);
    Ok((rel, prerelease))
}

/// "owner/name", trimmed, from either that form or a github.com link.
pub fn valid_repo(raw: &str) -> Result<String> {
    let s = raw.trim().trim_end_matches('/').trim_end_matches(".git");
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let s = s.strip_prefix("www.").unwrap_or(s);
    let s = match s.strip_prefix("github.com/") {
        Some(rest) => rest,
        // Another site's link ("gitlab.com/a/b") is not a GitHub project.
        None if s.split('/').next().is_some_and(|host| host.contains('.')) => bail!("Only GitHub projects can be added. Use owner/name or a github.com link."),
        None => s,
    };
    let parts: Vec<&str> = s.split('/').collect();
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && p != "." && p != "..";
    if parts.len() < 2 || !ok(parts[0]) || !ok(parts[1]) {
        bail!("That isn't a GitHub project. Use owner/name or a github.com link.");
    }
    Ok(format!("{}/{}", parts[0], parts[1]))
}

/// The catalog id for a project added from GitHub: owner and name joined by "--" (GitHub
/// names never contain "--" next to the slash, so two projects can't share one).
pub fn local_port_id(repo: &str) -> String {
    let part = |s: &str| s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect::<String>();
    let (owner, name) = repo.split_once('/').unwrap_or((repo, ""));
    format!("gh-{}--{}", part(owner), part(name))
}

/// Add a GitHub project to `catalog.local.json` as a port (other keys are kept). `game` is what
/// it is called in the library; `system` is an optional label for what it plays or is based on.
/// Adding a project that is already there replaces its entry.
pub fn add_local_port(path: &Path, root: &DataRoot, repo: &str, game: &str, system: Option<&str>) -> Result<PortDef> {
    let repo = valid_repo(repo)?;
    let game = game.trim();
    if game.is_empty() {
        bail!("Give it a name.");
    }
    if game.chars().count() > 120 {
        bail!("That name is too long.");
    }
    let (rel, prerelease) = newest(root, &repo)?;
    let rel = rel.ok_or_else(|| anyhow!("{repo} has not published any releases to install."))?;
    github::pick_asset(&rel, "auto").with_context(|| format!("{repo} has no Windows download in its newest release."))?;
    let port = PortDef {
        id: local_port_id(&repo),
        game: game.to_string(),
        name: repo.split('/').nth(1).unwrap_or(&repo).to_string(),
        kind: "Added by you".into(),
        original: system.map(str::trim).filter(|s| !s.is_empty()).map(|s| vec![s.to_string()]).unwrap_or_default(),
        repo,
        asset: "auto".into(),
        prerelease,
        exe: None,
        configure_exe: None,
        persist: vec![],
        carry: vec![],
        requires: None,
        licence: None,
        blurb: None,
        art: None,
        accent: None,
        featured: false,
    };
    let mut v = crate::catalog::read_local(path)?;
    let list = v
        .as_object_mut()
        .ok_or_else(|| anyhow!("catalog.local.json is not an object"))?
        .entry("ports")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow!("\"ports\" in catalog.local.json is not a list"))?;
    // The same id for a different project (names differing only in "." vs "_") would take over
    // the other one's install folder.
    if let Some(other) = list
        .iter()
        .filter(|p| p.get("id").and_then(|i| i.as_str()) == Some(port.id.as_str()))
        .filter_map(|p| p.get("repo").and_then(|r| r.as_str()))
        .find(|r| !r.eq_ignore_ascii_case(&port.repo))
    {
        bail!("{other} is already in your list under the same name. Remove it first to add {}.", port.repo);
    }
    list.retain(|p| p.get("id").and_then(|i| i.as_str()) != Some(port.id.as_str()));
    list.push(serde_json::to_value(&port)?);
    std::fs::write(path, serde_json::to_string_pretty(&v)?)?;
    Ok(port)
}

/// Remove a project added from GitHub. Built-in catalog entries can't be removed.
pub fn remove_local_port(path: &Path, id: &str) -> Result<()> {
    if !id.starts_with("gh-") {
        bail!("Only things you added yourself can be removed from the list.");
    }
    let mut v = crate::catalog::read_local(path)?;
    if let Some(list) = v.get_mut("ports").and_then(|l| l.as_array_mut()) {
        list.retain(|p| p.get("id").and_then(|i| i.as_str()) != Some(id));
    }
    std::fs::write(path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repos() {
        assert_eq!(valid_repo("HarbourMasters/Shipwright").unwrap(), "HarbourMasters/Shipwright");
        assert_eq!(valid_repo(" https://github.com/Zelda64Recomp/Zelda64Recomp/ ").unwrap(), "Zelda64Recomp/Zelda64Recomp");
        assert_eq!(valid_repo("github.com/a/b.git").unwrap(), "a/b");
        assert_eq!(valid_repo("https://github.com/a/b/releases/latest").unwrap(), "a/b");
        for bad in ["", "shipwright", "a/", "/b", "a b/c", "../x", "a/..", "gitlab.com/a/b", "https://example.com/x/y"] {
            assert!(valid_repo(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ids() {
        assert_eq!(local_port_id("HarbourMasters/Shipwright"), "gh-harbourmasters--shipwright");
        assert_eq!(local_port_id("a_b/c.d"), "gh-a_b--c_d");
        assert_ne!(local_port_id("a/b-c"), local_port_id("a-b/c"));
    }
}
