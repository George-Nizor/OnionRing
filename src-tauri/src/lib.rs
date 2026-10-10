//! Tauri shell: exposes the core to the UI as commands, streams progress as events.

use onionring_core::catalog::{Catalog, EmulatorDef, Platform, PortDef};
use onionring_core::db::Db;
use onionring_core::install::{self, ArchiveMode, InstalledApp, Target, UpdateInfo};
use onionring_core::model::{Copy, Emulator, Game, Progress, ScanSummary, Source};
use onionring_core::paths::{AppConfig, DataRoot, suggest_data_root, suggest_emulation_dirs};
use onionring_core::{art, launch, scan, storage, util};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[derive(Default)]
struct Shared {
    root: Option<DataRoot>,
}

struct AppState {
    shared: Mutex<Shared>,
    catalog: Mutex<Arc<Catalog>>,
    /// Serialises scans/installs so two never write the library at once.
    work: Mutex<()>,
}

impl AppState {
    fn root(&self) -> Res<DataRoot> {
        self.shared.lock().unwrap().root.clone().ok_or_else(|| "Choose a data folder first.".to_string())
    }
    fn db(&self) -> Res<Db> {
        Db::open(&self.root()?.db()).map_err(err)
    }
    fn cat(&self) -> Arc<Catalog> {
        self.catalog.lock().unwrap().clone()
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

fn emit_progress(app: &AppHandle) -> impl Fn(Progress) + Sync + use<> {
    let app = app.clone();
    move |p: Progress| {
        let _ = app.emit("progress", p);
    }
}

fn use_root(app: &AppHandle, root: DataRoot) -> Res<()> {
    app.asset_protocol_scope().allow_directory(&root.root, true).map_err(err)?;
    let cat = Catalog::load(Some(&root.local_catalog())).map_err(err)?;
    let st = app.state::<AppState>();
    *st.catalog.lock().unwrap() = Arc::new(cat);
    st.shared.lock().unwrap().root = Some(root);
    Ok(())
}

// ---- bootstrap & settings ------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    version: String,
    data_root: Option<String>,
    suggested_root: String,
    suggested_sources: Vec<String>,
    platforms: Vec<Platform>,
}

#[tauri::command]
fn bootstrap(app: AppHandle, st: State<AppState>) -> Res<Bootstrap> {
    Ok(Bootstrap {
        version: app.package_info().version.to_string(),
        data_root: st.shared.lock().unwrap().root.as_ref().map(|r| util::path_str(&r.root)),
        suggested_root: util::path_str(&suggest_data_root()),
        suggested_sources: suggest_emulation_dirs().iter().map(|p| util::path_str(p)).collect(),
        platforms: st.cat().platforms.clone(),
    })
}

#[tauri::command]
fn set_data_root(app: AppHandle, path: String) -> Res<()> {
    let root = DataRoot::open(PathBuf::from(&path)).map_err(err)?;
    AppConfig { data_root: Some(root.root.clone()) }.save().map_err(err)?;
    use_root(&app, root)
}

#[tauri::command]
fn sources(st: State<AppState>) -> Res<Vec<Source>> {
    st.db()?.sources().map_err(err)
}

#[tauri::command]
fn add_source(st: State<AppState>, path: String) -> Res<()> {
    st.db()?.add_source(path.trim_end_matches(['\\', '/'])).map_err(err)
}

#[tauri::command]
fn remove_source(st: State<AppState>, id: i64) -> Res<()> {
    st.db()?.remove_source(id).map_err(err)
}

// ---- library -------------------------------------------------------------------------------

#[tauri::command]
fn games(st: State<AppState>) -> Res<Vec<Game>> {
    st.db()?.games().map_err(err)
}

#[tauri::command]
fn game_copies(st: State<AppState>, key: String) -> Res<Vec<Copy>> {
    st.db()?.copies(&key).map_err(err)
}

#[tauri::command]
fn set_flag(st: State<AppState>, id: i64, flag: String, on: bool) -> Res<()> {
    st.db()?.set_flag(id, &flag, on).map_err(err)
}

#[tauri::command]
fn set_game_emulator(st: State<AppState>, id: i64, emulator: Option<String>) -> Res<()> {
    st.db()?.set_user_emulator(id, emulator.as_deref()).map_err(err)
}

#[tauri::command]
fn set_cover(st: State<AppState>, id: i64, path: String) -> Res<()> {
    art::set_custom_cover(&st.root()?, &st.db()?, id, &PathBuf::from(path)).map_err(err)
}

fn run_scan(app: &AppHandle) -> Res<ScanSummary> {
    let st = app.state::<AppState>();
    let _guard = st.work.lock().unwrap();
    let (root, cat, db) = (st.root()?, st.cat(), st.db()?);
    let progress = emit_progress(app);
    let started = util::now();
    let summary = scan::scan(&root, &cat, &db, &progress).map_err(err)?;
    db.kv_set("last_scan", &started.to_string()).map_err(err)?;
    let _ = app.emit("library-changed", ());
    art::refresh(&root, &cat, &db, false, &progress).map_err(err)?;
    let _ = app.emit("library-changed", ());
    progress(Progress::new("idle", "", 1, 1));
    Ok(summary)
}

#[tauri::command]
async fn scan_library(app: AppHandle) -> Res<ScanSummary> {
    blocking(move || run_scan(&app)).await
}

#[tauri::command]
async fn refresh_art(app: AppHandle, force: bool) -> Res<usize> {
    blocking(move || {
        let st = app.state::<AppState>();
        let _guard = st.work.lock().unwrap();
        let progress = emit_progress(&app);
        let n = art::refresh(&st.root()?, &st.cat(), &st.db()?, force, &progress).map_err(err)?;
        progress(Progress::new("idle", "", 1, 1));
        let _ = app.emit("library-changed", ());
        Ok(n)
    })
    .await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SessionEvent {
    id: i64,
    seconds: i64,
}

fn start(app: &AppHandle, id: i64, plan: launch::Plan, track: bool) -> Res<()> {
    let mut child = launch::spawn(&plan).map_err(err)?;
    let started = util::now();
    let db = app.state::<AppState>().db()?;
    db.mark_played(id, started).map_err(err)?;
    let _ = app.emit("game-started", id);
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        let ended = util::now();
        if track
            && let Ok(db) = app.state::<AppState>().db() {
                let _ = db.record_session(id, started, ended);
            }
        let _ = app.emit("game-ended", SessionEvent { id, seconds: ended - started });
        let _ = app.emit("library-changed", ());
    });
    Ok(())
}

#[tauri::command]
fn launch_game(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    let db = st.db()?;
    let game = db.game(id).map_err(err)?;
    let plan = launch::plan(&db, &st.cat(), &game).map_err(err)?;
    start(&app, id, plan, true)
}

#[tauri::command]
fn configure_port(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    let db = st.db()?;
    let game = db.game(id).map_err(err)?;
    let app_id = game.app_id.ok_or("not a port")?;
    let root = st.root()?;
    let inst = install::installed(&root, &app_id).ok_or("not installed")?;
    let exe = inst.configure_path(&root).or_else(|| inst.exe_path(&root)).ok_or("program missing")?;
    let cwd = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    start(&app, id, launch::Plan { program: exe, args: vec![], cwd }, false)
}

#[tauri::command]
async fn unpack_game(app: AppHandle, id: i64) -> Res<ScanSummary> {
    blocking(move || {
        {
            let st = app.state::<AppState>();
            let _ = app.emit("progress", Progress::new("unpack", "Unpacking", 0, 1));
            storage::unpack(&st.db()?, id).map_err(err)?;
        }
        run_scan(&app)
    })
    .await
}

// ---- emulators & ports ---------------------------------------------------------------------

#[tauri::command]
fn emulators(st: State<AppState>) -> Res<Vec<Emulator>> {
    let db = st.db()?;
    let cutoff = db.kv_get("last_scan").map_err(err)?.and_then(|s| s.parse().ok()).unwrap_or(0);
    launch::emulators(&db, &st.cat(), cutoff).map_err(err)
}

#[tauri::command]
fn set_emulator_args(st: State<AppState>, id: String, args: Option<Vec<String>>) -> Res<()> {
    st.db()?.set_emulator_args(&id, args.as_deref()).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogView {
    ports: Vec<PortDef>,
    emulators: Vec<EmulatorDef>,
    installed: Vec<InstalledApp>,
}

#[tauri::command]
fn catalog(st: State<AppState>) -> Res<CatalogView> {
    let cat = st.cat();
    let installed = st.shared.lock().unwrap().root.as_ref().map(install::installed_apps).unwrap_or_default();
    Ok(CatalogView { ports: cat.ports.clone(), emulators: cat.emulators.clone(), installed })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallEvent {
    id: String,
    message: String,
    done: bool,
    error: Option<String>,
}

fn install_with(
    app: &AppHandle,
    id: &str,
    f: impl FnOnce(&DataRoot, &Target, &dyn Fn(&str)) -> anyhow::Result<InstalledApp>,
) -> Res<InstalledApp> {
    let st = app.state::<AppState>();
    let root = st.root()?;
    let target = Target::from_catalog(&st.cat(), id).map_err(err)?;
    let report = |m: &str| {
        let _ = app.emit("install", InstallEvent { id: id.to_string(), message: m.to_string(), done: false, error: None });
    };
    let result = {
        let _guard = st.work.lock().unwrap();
        f(&root, &target, &report)
    };
    match result {
        Ok(a) => {
            let _ = app.emit("install", InstallEvent { id: id.to_string(), message: "Installed".into(), done: true, error: None });
            let _ = run_scan(app);
            Ok(a)
        }
        Err(e) => {
            let msg = err(e);
            let _ = app.emit("install", InstallEvent { id: id.to_string(), message: msg.clone(), done: true, error: Some(msg.clone()) });
            Err(msg)
        }
    }
}

#[tauri::command]
async fn install_latest(app: AppHandle, id: String) -> Res<InstalledApp> {
    blocking(move || install_with(&app, &id, |r, t, p| install::install_latest(r, t, p))).await
}

#[tauri::command]
async fn install_from_file(app: AppHandle, id: String, path: String) -> Res<InstalledApp> {
    blocking(move || {
        install_with(&app, &id, |r, t, p| install::install_archive(r, t, &PathBuf::from(&path), ArchiveMode::Copy, None, p))
    })
    .await
}

#[tauri::command]
async fn check_updates(app: AppHandle) -> Res<Vec<UpdateInfo>> {
    blocking(move || {
        let st = app.state::<AppState>();
        let (root, cat) = (st.root()?, st.cat());
        let mut out = vec![];
        for a in install::installed_apps(&root) {
            if let Ok(t) = Target::from_catalog(&cat, &a.id)
                && let Ok(Some(u)) = install::check_update(&root, &t) {
                    out.push(u);
                }
        }
        Ok(out)
    })
    .await
}

// ---- storage -------------------------------------------------------------------------------

#[tauri::command]
async fn storage_report(app: AppHandle) -> Res<storage::Report> {
    blocking(move || {
        let st = app.state::<AppState>();
        storage::report(&st.root()?, &st.db()?).map_err(err)
    })
    .await
}

#[tauri::command]
async fn reclaim(app: AppHandle, paths: Vec<String>) -> Res<(usize, i64)> {
    blocking(move || {
        let st = app.state::<AppState>();
        let r = {
            let _guard = st.work.lock().unwrap();
            storage::reclaim(&st.root()?, &st.db()?, &paths).map_err(err)?
        };
        let _ = app.emit("library-changed", ());
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn restore_reclaimed(app: AppHandle) -> Res<usize> {
    blocking(move || {
        let n = storage::restore_all(&app.state::<AppState>().root()?).map_err(err)?;
        run_scan(&app)?;
        Ok(n)
    })
    .await
}

#[tauri::command]
fn reveal(app: AppHandle, path: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(path).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            shared: Mutex::new(Shared::default()),
            catalog: Mutex::new(Arc::new(Catalog::load(None).expect("built-in catalog is valid"))),
            work: Mutex::new(()),
        })
        .setup(|app| {
            if let Some(path) = AppConfig::load().data_root {
                match DataRoot::open(path) {
                    Ok(root) => use_root(app.handle(), root)?,
                    Err(e) => eprintln!("data root unavailable: {e:#}"),
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            set_data_root,
            sources,
            add_source,
            remove_source,
            games,
            game_copies,
            set_flag,
            set_game_emulator,
            set_cover,
            scan_library,
            refresh_art,
            launch_game,
            configure_port,
            unpack_game,
            emulators,
            set_emulator_args,
            catalog,
            install_latest,
            install_from_file,
            check_updates,
            storage_report,
            reclaim,
            restore_reclaimed,
            reveal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OnionRing");
}
