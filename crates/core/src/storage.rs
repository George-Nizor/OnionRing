//! Storage report: the same game kept several times (an archive next to its extracted copy, an
//! ISO next to an extracted folder). Reclaiming *moves* redundant copies into
//! `<data root>\Reclaim\<timestamp>\` on the same drive (instant, reversible). Nothing is ever
//! deleted by OnionRing; emptying Reclaim is left to the user.

use crate::db::Db;
use crate::install;
use crate::paths::{DataRoot, same_volume};
use crate::util::{now, path_str, split_name};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Redundant {
    pub path: String,
    pub size: i64,
    pub format: String,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub key: String,
    pub title: String,
    pub platform: String,
    pub keep: String,
    pub keep_size: i64,
    pub redundant: Vec<Redundant>,
    pub reclaimable: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub library_size: i64,
    pub reclaimable: i64,
    pub groups: Vec<Group>,
    pub reclaim_dir: String,
    pub held: i64,
}

pub fn report(root: &DataRoot, db: &Db) -> Result<Report> {
    let mut by_game: BTreeMap<String, Vec<(String, String, crate::model::Copy)>> = BTreeMap::new();
    for (key, title, platform, c) in db.all_copies()? {
        by_game.entry(key).or_default().push((title, platform, c));
    }
    let mut groups = Vec::new();
    let mut library_size = 0;
    for (key, copies) in by_game {
        let Some((title, platform, primary)) = copies.iter().find(|(_, _, c)| c.primary) else { continue };
        library_size += primary.size;
        if primary.archive {
            continue; // only the archive exists; nothing is redundant
        }
        let redundant: Vec<Redundant> = copies
            .iter()
            .filter(|(_, _, c)| !c.primary)
            .map(|(_, _, c)| Redundant {
                path: c.path.clone(),
                size: c.size,
                format: c.format.clone(),
                reason: if c.archive {
                    "Archive the playable copy was unpacked from".into()
                } else {
                    "Second copy of a game that is already in your library".into()
                },
            })
            .collect();
        if redundant.is_empty() {
            continue;
        }
        groups.push(Group {
            key,
            title: title.clone(),
            platform: platform.clone(),
            keep: primary.path.clone(),
            keep_size: primary.size,
            reclaimable: redundant.iter().map(|r| r.size).sum(),
            redundant,
        });
    }
    groups.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable));
    Ok(Report {
        library_size,
        reclaimable: groups.iter().map(|g| g.reclaimable).sum(),
        groups,
        reclaim_dir: path_str(&root.reclaim()),
        held: crate::util::dir_size(&root.reclaim()) as i64,
    })
}

#[derive(Serialize, Deserialize, Default)]
struct RestoreLog {
    moved: Vec<(String, String)>,
}

/// Move redundant copies out of the way. Only paths the report lists as redundant are accepted.
pub fn reclaim(root: &DataRoot, db: &Db, paths: &[String]) -> Result<(usize, i64)> {
    let rep = report(root, db)?;
    let allowed: BTreeMap<&str, i64> =
        rep.groups.iter().flat_map(|g| g.redundant.iter().map(|r| (r.path.as_str(), r.size))).collect();
    let batch = root.reclaim().join(now().to_string());
    let mut log = RestoreLog::default();
    let (mut n, mut bytes) = (0, 0);
    for p in paths {
        let Some(size) = allowed.get(p.as_str()) else { bail!("{p} is not a redundant copy") };
        let src = PathBuf::from(p);
        if !same_volume(&src, &batch) {
            bail!("{p} is on a different drive from your data folder. Open its folder and delete it yourself if you are sure.");
        }
        // Keep only the plain folder names; a leading `\` or drive would make join() discard `batch`.
        let rel: PathBuf = src.components().filter(|c| matches!(c, std::path::Component::Normal(_))).collect();
        let dest = batch.join(rel);
        if !dest.starts_with(&batch) || dest.exists() {
            bail!("refusing to move {p}");
        }
        std::fs::create_dir_all(dest.parent().context("bad path")?)?;
        std::fs::rename(&src, &dest).with_context(|| format!("moving {p}"))?;
        db.delete_copy(p)?;
        log.moved.push((p.clone(), path_str(&dest)));
        n += 1;
        bytes += size;
        std::fs::write(batch.join("restore.json"), serde_json::to_string_pretty(&log)?)?;
    }
    Ok((n, bytes))
}

/// Put everything in Reclaim back where it came from.
pub fn restore_all(root: &DataRoot) -> Result<usize> {
    let mut n = 0;
    let Ok(rd) = std::fs::read_dir(root.reclaim()) else { return Ok(0) };
    for batch in rd.filter_map(|e| e.ok()) {
        let Ok(s) = std::fs::read_to_string(batch.path().join("restore.json")) else { continue };
        let log: RestoreLog = serde_json::from_str(&s)?;
        for (orig, moved) in log.moved {
            let (o, m) = (Path::new(&orig), Path::new(&moved));
            if m.exists() && !o.exists() {
                if let Some(parent) = o.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(m, o)?;
                n += 1;
            }
        }
        let _ = std::fs::remove_file(batch.path().join("restore.json"));
        let _ = remove_empty_dirs(&batch.path());
    }
    Ok(n)
}

fn remove_empty_dirs(dir: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)?.flatten() {
        if e.file_type()?.is_dir() {
            let _ = remove_empty_dirs(&e.path());
        }
    }
    std::fs::remove_dir(dir)
}

/// Unpack a game that only exists as an archive into a folder beside it.
pub fn unpack(db: &Db, id: i64) -> Result<PathBuf> {
    let g = db.game(id)?;
    if !g.packed {
        bail!("{} is already unpacked", g.title);
    }
    let archive = PathBuf::from(&g.launch_path);
    let (stem, _) = split_name(&archive);
    let dest = archive.parent().context("bad path")?.join(stem);
    install::extract(&archive, &dest)?;
    Ok(dest)
}
