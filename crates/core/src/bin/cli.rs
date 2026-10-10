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
    let cat = Catalog::load(Some(&root.local_catalog()))?;
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
        ["snapshot", out] => {
            // Everything the UI loads, as one JSON file, for previewing the UI in a browser.
            let (root, cat, db) = open()?;
            let snap = serde_json::json!({
                "bootstrap": {
                    "version": env!("CARGO_PKG_VERSION"),
                    "dataRoot": root.root,
                    "suggestedRoot": root.root,
                    "suggestedSources": onionring_core::paths::suggest_emulation_dirs(),
                    "platforms": cat.platforms,
                },
                "games": db.games()?,
                "emulators": launch::emulators(&db, &cat, 0)?,
                "sources": db.sources()?,
                "catalog": { "ports": cat.ports, "emulators": cat.emulators, "installed": install::installed_apps(&root) },
                "storage": storage::report(&root, &db)?,
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
            eprintln!("{}", include_str!("cli.rs").lines().skip(2).take(11).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
        }
    }
    Ok(())
}
