//! Headless OnionRing: the same core the app uses, for scripting and debugging.
//!
//! ```text
//! onionring-cli root [<path>]              show or set the data root
//! onionring-cli source add|remove|list [<path>]
//! onionring-cli scan                        scan sources, then fetch missing art
//! onionring-cli art [--force]
//! onionring-cli games
//! onionring-cli emulators
//! onionring-cli install <id> [--archive <file> [--move]]
//! onionring-cli mashups                  each mashup, its game and what it still needs
//! onionring-cli mashup remove <id>        take a mashup out of its game and uninstall it
//! onionring-cli catalog refresh          fetch the live catalog from main
//! onionring-cli updates
//! onionring-cli storage
//! ```

use anyhow::{Context, Result, bail};
use onionring_core::catalog::Catalog;
use onionring_core::db::Db;
use onionring_core::install::{self, ArchiveMode, Target};
use onionring_core::model::Progress;
use onionring_core::paths::{AppConfig, DataRoot, suggest_data_root};
use onionring_core::{art, launch, scan, storage};
use std::path::PathBuf;

fn gb(b: i64) -> String {
    format!("{:.2} GB", b as f64 / 1e9)
}

fn open() -> Result<(DataRoot, Catalog, Db)> {
    let cfg = AppConfig::load();
    let root = DataRoot::open(cfg.data_root.context("no data root yet: run `onionring-cli root <path>`")?)?;
    let cat = Catalog::load_layers(onionring_core::catalog::live_layer(&root).as_deref(), Some(&root.local_catalog()))?;
    let db = Db::open(&root.db())?;
    Ok((root, cat, db))
}

fn show(p: Progress) {
    if p.stage == "art" || p.stage == "save" {
        if p.done.is_multiple_of(10) || p.done == p.total {
            eprintln!("  [{}] {}/{} {}", p.stage, p.done, p.total, p.detail);
        }
    } else {
        eprintln!("  [{}] {}", p.stage, p.detail);
    }
}

fn cached_pictures(root: &DataRoot, cat: &Catalog) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    let mut add = |kind: &str, id: &str| {
        if let Some(p) = onionring_core::pictures::cached(root, kind, id) {
            m.insert(format!("{kind}:{id}"), p.display().to_string().into());
        }
    };
    cat.platforms.iter().for_each(|p| add("system", &p.id));
    cat.emulators.iter().for_each(|e| add("emulator", &e.id));
    cat.browser.iter().for_each(|b| add("browser", &b.id));
    cat.ports.iter().for_each(|p| add("port", &p.id));
    cat.mashups.iter().for_each(|m| add("mashup", &m.id));
    m
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match a.as_slice() {
        ["root"] => {
            let cfg = AppConfig::load();
            println!("data root: {}", cfg.data_root.map(|p| p.display().to_string()).unwrap_or_else(|| "(not set)".into()));
            println!("suggested: {}", suggest_data_root().display());
        }
        ["root", path] => {
            let root = DataRoot::open(PathBuf::from(path))?;
            AppConfig { data_root: Some(root.root.clone()) }.save()?;
            println!("data root set to {}", root.root.display());
        }
        ["anyps5", "install"] => {
            let (root, _, db) = open()?;
            let v = onionring_core::anyps5::install_latest(&db, &root, &|m| println!("  {m}"))?;
            println!("AnyPS5 {v} installed");
        }
        ["anyps5", "convert", id] => {
            let (root, _, db) = open()?;
            let game = db.game(id.parse()?)?;
            let exe = onionring_core::anyps5::convert(&db, &root, &game, &|m| println!("  {m}"))?;
            println!("{}", exe.display());
        }
        ["source", "list"] => {
            let (_, _, db) = open()?;
            for s in db.sources()? {
                println!("{:>3}  {}{}", s.id, s.path, if s.exists { "" } else { "  (missing)" });
            }
        }
        ["source", "add", path] => {
            let (_, _, db) = open()?;
            let p = std::fs::canonicalize(path).map(|p| p.display().to_string().trim_start_matches(r"\\?\").to_string())?;
            db.add_source(&p)?;
            println!("added {p}");
        }
        ["source", "remove", id] => {
            let (_, _, db) = open()?;
            db.remove_source(id.parse()?)?;
        }
        ["scan"] => {
            let (root, cat, db) = open()?;
            let s = scan::scan(&root, &cat, &db, &show)?;
            println!("{} games ({} new), {} emulators, {} copies, {} missing, {} ms", s.games, s.new_games, s.emulators, s.copies, s.missing, s.millis);
            let n = art::refresh(&root, &cat, &db, false, &show)?;
            println!("art found for {n} games");
        }
        ["art", rest @ ..] => {
            let (root, cat, db) = open()?;
            let n = art::refresh(&root, &cat, &db, rest.contains(&"--force"), &show)?;
            println!("art found for {n} games");
        }
        ["games"] => {
            let (_, _, db) = open()?;
            for g in db.games()? {
                println!(
                    "{:>4}  {:<5} {:<46} {:<10} {:>9}  copies={} {}{}{}",
                    g.id,
                    g.platform,
                    g.title.chars().take(46).collect::<String>(),
                    g.emulator.clone().unwrap_or_else(|| "-".into()),
                    gb(g.size),
                    g.copies,
                    if g.packed { "[packed] " } else { "" },
                    if g.cover.is_some() { "[cover] " } else { "" },
                    if g.hero.is_some() { "[hero]" } else { "" },
                );
            }
        }
        ["emulators"] => {
            let (_, cat, db) = open()?;
            for e in launch::emulators(&db, &cat, 0)? {
                println!("{:<10} {:<10} {:<12} {}  args={:?}", e.id, e.version.unwrap_or_default(), e.platforms.join(","), e.exe, e.args);
            }
        }
        ["install", id, rest @ ..] => {
            let (root, cat, _db) = open()?;
            let t = Target::from_catalog(&cat, id)?;
            let report = |s: &str| eprintln!("  {s}");
            let app = match rest {
                ["--archive", file, more @ ..] => {
                    let mode = if more.contains(&"--move") { ArchiveMode::Move } else { ArchiveMode::Copy };
                    install::install_archive(&root, &t, &PathBuf::from(file), mode, None, &report)?
                }
                [] => install::install_latest(&root, &t, &report)?,
                _ => bail!("usage: install <id> [--archive <file> [--move]]"),
            };
            println!("{}", serde_json::to_string_pretty(&app)?);
        }
        ["mashups"] => {
            let (root, cat, _db) = open()?;
            for s in onionring_core::mashups::states(&root, &cat) {
                let installed = install::installed(&root, &s.id).map(|a| a.version).unwrap_or_else(|| "-".into());
                let host = match &s.host.dir {
                    Some(d) => format!("{} at {d}{}", s.host.name, if s.host.chosen { " (chosen)" } else { "" }),
                    None => format!("{} not installed", s.host.name),
                };
                println!("{:<12} {:<10} {host}{}", s.id, installed, if s.deployed { "  [in the game]" } else { "" });
                for n in s.needs {
                    println!("    {} {}", if n.ok { "ok     " } else { "MISSING" }, n.what);
                }
            }
        }
        ["mashup", "remove", id] => {
            let (root, cat, db) = open()?;
            cat.mashup(id).with_context(|| format!("{id} is not a mashup"))?;
            onionring_core::mashups::remove(&root, id)?;
            db.forget_game(&format!("mashup:{id}"))?;
            println!("removed {id}");
        }
        ["catalog", "refresh"] => {
            let (root, _, _db) = open()?;
            let changed = onionring_core::catalog::refresh_live(&root.live_catalog())?;
            let cat = Catalog::load_layers(onionring_core::catalog::live_layer(&root).as_deref(), Some(&root.local_catalog()))?;
            println!("{} ({} ports, {} emulators)", if changed { "updated" } else { "already current" }, cat.ports.len(), cat.emulators.len());
        }
        ["updates"] => {
            let (root, cat, _db) = open()?;
            for app in install::installed_apps(&root) {
                let t = Target::from_catalog(&cat, &app.id)?;
                match install::check_update(&root, &t) {
                    Ok(Some(u)) => println!("{:<20} {} -> {}{}", u.id, u.installed, u.latest, if u.available { "  UPDATE" } else { "" }),
                    Ok(None) => {}
                    Err(e) => println!("{:<20} error: {e:#}", app.id),
                }
            }
        }
        ["pictures", rest @ ..] => {
            // Prefetch pictures: systems, emulators, browser games, mashups, and the first N ports
            // ("all" for every port). --refresh forgets port and browser pictures (and misses)
            // first, so they are looked up again; system and emulator pictures are kept.
            use onionring_core::pictures;
            use rayon::prelude::*;
            let (root, cat, _db) = open()?;
            if let ["--explain", kind, id] = rest {
                pictures::explain(&root, &cat, kind, id).iter().for_each(|l| println!("{l}"));
                return Ok(());
            }
            if rest.contains(&"--refresh") {
                let n = pictures::forget(&root, &["port", "browser"])?;
                println!("forgot {n} port and browser pictures");
            }
            let n: usize = rest.iter().find_map(|s| if *s == "all" { Some(usize::MAX) } else { s.parse().ok() }).unwrap_or(24);
            let mut jobs: Vec<(&str, String)> = Vec::new();
            jobs.extend(cat.platforms.iter().filter(|p| !p.extensions.is_empty()).map(|p| ("system", p.id.clone())));
            jobs.extend(cat.emulators.iter().map(|e| ("emulator", e.id.clone())));
            jobs.extend(cat.browser.iter().map(|b| ("browser", b.id.clone())));
            jobs.extend(cat.ports.iter().take(n).map(|p| ("port", p.id.clone())));
            jobs.extend(cat.mashups.iter().map(|m| ("mashup", m.id.clone())));
            // A picture found to be generic is taken off the games that already had it, so go
            // round again until nothing changes.
            let have = |jobs: &[(&str, String)]| jobs.iter().filter(|(k, id)| pictures::cached(&root, k, id).is_some()).count();
            let mut before = usize::MAX;
            for _ in 0..4 {
                jobs.par_iter().for_each(|(k, id)| {
                    if let Err(e) = pictures::picture(&root, &cat, k, id) {
                        eprintln!("  {k} {id}: {e:#}");
                    }
                });
                let now = have(&jobs);
                if now == before {
                    break;
                }
                before = now;
            }
            for kind in ["system", "emulator", "browser", "port", "mashup"] {
                let of: Vec<_> = jobs.iter().filter(|(k, _)| *k == kind).cloned().collect();
                println!("{kind:<9} {} of {} have pictures", have(&of), of.len());
            }
            println!("generic images banned: {}", pictures::generic_images(&root));
            // Ports and browser games must not share a picture.
            let mut by_hash: std::collections::BTreeMap<String, Vec<String>> = Default::default();
            for (k, id) in jobs.iter().filter(|(k, _)| *k == "port" || *k == "browser") {
                if let Some(p) = pictures::cached(&root, k, id) {
                    by_hash.entry(onionring_core::util::sha256_file(&p)?).or_default().push(format!("{k}:{id}"));
                }
            }
            let shared: Vec<_> = by_hash.values().filter(|v| v.len() > 1).collect();
            if shared.is_empty() {
                println!("no two ports or browser games share a picture");
            }
            for v in shared {
                println!("shared picture: {}", v.join(", "));
            }
        }
        ["snapshot", out] => {
            // Everything the UI loads, as one JSON file, for previewing the UI in a browser.
            use onionring_core::{backups, checks, updates, util};
            let (root, cat, db) = open()?;
            let games = db.games()?;
            let emulators = launch::emulators(&db, &cat, 0)?;
            // Every game's sessions (newest first), so the preview can answer any history query.
            let mut sessions = serde_json::Map::new();
            for g in &games {
                let s = db.game_sessions(g.id, 10_000)?;
                if !s.is_empty() {
                    sessions.insert(g.id.to_string(), serde_json::to_value(s)?);
                }
            }
            let updates = updates::available_updates(&root, &cat, &db).unwrap_or_else(|e| {
                eprintln!("  updates left out: {e:#}");
                vec![]
            });
            let save_sources = backups::save_sources(&root, &db, &cat)?;
            // Folder sizes stand in for archive sizes when the preview pretends to back up.
            let save_sizes: serde_json::Map<String, serde_json::Value> =
                save_sources.iter().map(|s| (s.path.clone(), util::dir_size(std::path::Path::new(&s.path)).into())).collect();
            let snap = serde_json::json!({
                "bootstrap": {
                    "version": env!("CARGO_PKG_VERSION"),
                    "dataRoot": root.root,
                    "suggestedRoot": root.root,
                    "suggestedSources": onionring_core::paths::suggest_emulation_dirs(),
                    "platforms": cat.platforms,
                },
                "checks": checks::emulator_checks(&cat, &emulators),
                "games": games,
                "emulators": emulators,
                "sources": db.sources()?,
                "catalog": { "ports": cat.ports, "emulators": cat.emulators, "browser": cat.browser, "mashups": cat.mashups, "mashupStates": onionring_core::mashups::states(&root, &cat), "installed": install::installed_apps(&root) },
                "storage": storage::report(&root, &db)?,
                "pictures": cached_pictures(&root, &cat),
                "updates": updates,
                "collections": db.collections()?,
                // Weekly totals are worked out by the preview from the sessions, so they follow
                // today's date rather than the day the snapshot was taken.
                "history": {
                    "totals": db.play_totals()?,
                    "top": db.top_games(10_000, None)?,
                    "sessions": sessions,
                },
                "saveSources": save_sources,
                "saveSizes": save_sizes,
                "backups": backups::list_backups(&root),
                // Same kv key and default (on) as the app's auto-pickup setting.
                "autoScan": db.kv_get("auto_scan")?.as_deref() != Some("0"),
            });
            std::fs::write(out, serde_json::to_string(&snap)?)?;
            println!("wrote {out}");
        }
        ["storage"] => {
            let (root, _, db) = open()?;
            let r = storage::report(&root, &db)?;
            println!("library {} | reclaimable {} | held in Reclaim {}", gb(r.library_size), gb(r.reclaimable), gb(r.held));
            for g in r.groups {
                println!("{} ({}) keep {}", g.title, g.platform, g.keep);
                for x in g.redundant {
                    println!("    {:>9}  {}  - {}", gb(x.size), x.path, x.reason);
                }
            }
        }
        _ => {
            eprintln!("{}", include_str!("cli.rs").lines().skip(2).take(14).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
        }
    }
    Ok(())
}
