//! Tauri shell: exposes the core to the UI as commands, streams progress as events.

use onionring_core::backups::{self, Backup, SaveSource};
use onionring_core::catalog::{BrowserGame, Catalog, EmulatorDef, Platform, PortDef};
use onionring_core::checks::{self, EmulatorChecks};
use onionring_core::db::Db;
use onionring_core::install::{self, ArchiveMode, InstalledApp, Target, UpdateInfo};
use onionring_core::model::{
    Collection, Copy, Emulator, Game, PlayTotals, Progress, ScanSummary, Session, Source, TopGame, WeekTotal,
};
use onionring_core::paths::{AppConfig, DataRoot, suggest_data_root, suggest_emulation_dirs};
use onionring_core::updates::{self, AvailableUpdate};
use onionring_core::watch::{self, PathKind};
use onionring_core::{art, launch, scan, storage, util};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

/// Lock a mutex, carrying on if a thread panicked while holding it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
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
    /// One save backup at a time.
    backup: Mutex<()>,
    /// Games (and port settings programs) running now. Automatic scans wait until none are.
    running: AtomicUsize,
    /// When the latest scan started, so a change it already saw doesn't cause another scan.
    scan_started: Mutex<Option<Instant>>,
    /// Auto-pickup: watches the library's folders while it is on.
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Bumped whenever the watcher is replaced; an older watch loop sees this and stops.
    watch_gen: AtomicU64,
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
    use_root(&app, root)?;
    rewatch(&app);
    Ok(())
}

#[tauri::command]
fn sources(st: State<AppState>) -> Res<Vec<Source>> {
    st.db()?.sources().map_err(err)
}

#[tauri::command]
fn add_source(app: AppHandle, st: State<AppState>, path: String) -> Res<()> {
    st.db()?.add_source(path.trim_end_matches(['\\', '/'])).map_err(err)?;
    rewatch(&app);
    Ok(())
}

#[tauri::command]
fn remove_source(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    st.db()?.remove_source(id).map_err(err)?;
    rewatch(&app);
    Ok(())
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

/// Launch options for one game (one argument per entry, same placeholders as the emulator's).
/// `None` or an empty list goes back to the emulator's own options.
#[tauri::command]
fn set_game_args(app: AppHandle, st: State<AppState>, id: i64, args: Option<Vec<String>>) -> Res<()> {
    st.db()?.set_game_args(id, args.as_deref()).map_err(err)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn set_cover(st: State<AppState>, id: i64, path: String) -> Res<()> {
    art::set_custom_cover(&st.root()?, &st.db()?, id, &PathBuf::from(path)).map_err(err)
}

// ---- collections ---------------------------------------------------------------------------

fn collections_changed(app: &AppHandle) {
    let _ = app.emit("collections-changed", ());
}

#[tauri::command]
fn collections(st: State<AppState>) -> Res<Vec<Collection>> {
    st.db()?.collections().map_err(err)
}

#[tauri::command]
fn create_collection(app: AppHandle, st: State<AppState>, name: String) -> Res<Collection> {
    let c = st.db()?.create_collection(&name).map_err(err)?;
    collections_changed(&app);
    Ok(c)
}

#[tauri::command]
fn rename_collection(app: AppHandle, st: State<AppState>, id: i64, name: String) -> Res<()> {
    st.db()?.rename_collection(id, &name).map_err(err)?;
    collections_changed(&app);
    Ok(())
}

#[tauri::command]
fn delete_collection(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    st.db()?.delete_collection(id).map_err(err)?;
    collections_changed(&app);
    Ok(())
}

#[tauri::command]
fn add_to_collection(app: AppHandle, st: State<AppState>, collection_id: i64, game_id: i64) -> Res<()> {
    st.db()?.add_to_collection(collection_id, game_id).map_err(err)?;
    collections_changed(&app);
    Ok(())
}

#[tauri::command]
fn remove_from_collection(app: AppHandle, st: State<AppState>, collection_id: i64, game_id: i64) -> Res<()> {
    st.db()?.remove_from_collection(collection_id, game_id).map_err(err)?;
    collections_changed(&app);
    Ok(())
}

// ---- play history --------------------------------------------------------------------------

/// A game's sessions, newest first (50 unless `limit` says otherwise).
#[tauri::command]
fn game_sessions(st: State<AppState>, id: i64, limit: Option<usize>) -> Res<Vec<Session>> {
    st.db()?.game_sessions(id, limit.unwrap_or(50).min(10_000)).map_err(err)
}

/// Time played per week (Monday to Sunday, local time) for the last `weeks` weeks (12 unless
/// given) including this one, oldest first, empty weeks included.
#[tauri::command]
fn weekly_playtime(st: State<AppState>, weeks: Option<usize>) -> Res<Vec<WeekTotal>> {
    st.db()?.weekly_playtime(weeks.unwrap_or(12).clamp(1, 520), util::now(), util::local_offset()).map_err(err)
}

#[tauri::command]
fn play_totals(st: State<AppState>) -> Res<PlayTotals> {
    st.db()?.play_totals().map_err(err)
}

/// Games with the most time played (10 unless `limit` says otherwise), optionally only counting
/// sessions that started at or after `since` (Unix seconds).
#[tauri::command]
fn top_games(st: State<AppState>, limit: Option<usize>, since: Option<i64>) -> Res<Vec<TopGame>> {
    st.db()?.top_games(limit.unwrap_or(10).min(10_000), since).map_err(err)
}

fn run_scan(app: &AppHandle) -> Res<ScanSummary> {
    let st = app.state::<AppState>();
    let _guard = lock(&st.work);
    scan_locked(app)
}

/// Scan, with `work` already held by the caller.
fn scan_locked(app: &AppHandle) -> Res<ScanSummary> {
    let st = app.state::<AppState>();
    *lock(&st.scan_started) = Some(Instant::now());
    let progress = emit_progress(app);
    let result = (|| {
        let (root, cat, db) = (st.root()?, st.cat(), st.db()?);
        let started = util::now();
        let summary = scan::scan(&root, &cat, &db, &progress).map_err(err)?;
        db.kv_set("last_scan", &started.to_string()).map_err(err)?;
        let _ = app.emit("library-changed", ());
        art::refresh(&root, &cat, &db, false, &progress).map_err(err)?;
        let _ = app.emit("library-changed", ());
        Ok(summary)
    })();
    // Also on failure, so the status bar doesn't stay stuck on a half-finished scan.
    progress(Progress::new("idle", "", 1, 1));
    result
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
    let st = app.state::<AppState>();
    st.running.fetch_add(1, Ordering::SeqCst);
    if let Err(e) = st.db().and_then(|db| db.mark_played(id, started).map_err(err)) {
        eprintln!("could not mark game {id} as played: {e}");
    }
    let _ = app.emit("game-started", id);
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        let ended = util::now();
        let st = app.state::<AppState>();
        if track
            && let Ok(db) = st.db() {
                let _ = db.record_session(id, started, ended);
            }
        st.running.fetch_sub(1, Ordering::SeqCst);
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

fn found_emulators(st: &AppState) -> Res<Vec<Emulator>> {
    let db = st.db()?;
    let cutoff = db.kv_get("last_scan").map_err(err)?.and_then(|s| s.parse().ok()).unwrap_or(0);
    launch::emulators(&db, &st.cat(), cutoff).map_err(err)
}

#[tauri::command]
fn emulators(st: State<AppState>) -> Res<Vec<Emulator>> {
    found_emulators(&st)
}

/// Files each emulator on the PC needs from the player (BIOS, firmware, keys), checked where
/// that copy of the emulator looks for them. Emulators that need nothing are left out.
#[tauri::command]
async fn setup_checks(app: AppHandle) -> Res<Vec<EmulatorChecks>> {
    blocking(move || {
        let st = app.state::<AppState>();
        let emus = found_emulators(&st)?;
        Ok(checks::emulator_checks(&st.cat(), &emus))
    })
    .await
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
    browser: Vec<BrowserGame>,
    installed: Vec<InstalledApp>,
}

#[tauri::command]
fn catalog(st: State<AppState>) -> Res<CatalogView> {
    let cat = st.cat();
    let installed = st.shared.lock().unwrap().root.as_ref().map(install::installed_apps).unwrap_or_default();
    Ok(CatalogView { ports: cat.ports.clone(), emulators: cat.emulators.clone(), browser: cat.browser.clone(), installed })
}

/// A picture for a system, emulator, port or browser game (fetched once, then cached).
#[tauri::command]
async fn picture(app: AppHandle, kind: String, id: String) -> Res<Option<String>> {
    blocking(move || {
        let st = app.state::<AppState>();
        let p = onionring_core::pictures::picture(&st.root()?, &st.cat(), &kind, &id).map_err(err)?;
        Ok(p.map(|p| util::path_str(&p)))
    })
    .await
}

/// Latest release of a port or emulator, and the download OnionRing would use.
#[tauri::command]
async fn release_details(app: AppHandle, id: String) -> Res<Option<install::ReleaseDetails>> {
    blocking(move || {
        let st = app.state::<AppState>();
        let t = Target::from_catalog(&st.cat(), &id).map_err(err)?;
        install::release_details(&st.root()?, &t).map_err(err)
    })
    .await
}

fn reload_catalog(app: &AppHandle) -> Res<()> {
    let st = app.state::<AppState>();
    let cat = Catalog::load(Some(&st.root()?.local_catalog())).map_err(err)?;
    *st.catalog.lock().unwrap() = Arc::new(cat);
    Ok(())
}

#[tauri::command]
fn add_browser_game(app: AppHandle, st: State<AppState>, title: String, url: String) -> Res<BrowserGame> {
    let g = onionring_core::catalog::add_local_browser_game(&st.root()?.local_catalog(), &title, &url).map_err(err)?;
    reload_catalog(&app)?;
    Ok(g)
}

#[tauri::command]
fn remove_browser_game(app: AppHandle, st: State<AppState>, id: String) -> Res<()> {
    onionring_core::catalog::remove_local_browser_game(&st.root()?.local_catalog(), &id).map_err(err)?;
    reload_catalog(&app)
}

/// The site a URL belongs to (last two host labels), so a game can move between its own
/// subdomains but any other link leaves for the user's browser.
fn site(u: &tauri::Url) -> String {
    let host = u.host_str().unwrap_or_default();
    let labels: Vec<&str> = host.split('.').collect();
    labels[labels.len().saturating_sub(2)..].join(".")
}

/// Open a browser game. "window" games get their own OnionRing window: no access to the app
/// (it is not in any capability), it stays on the game's site, popups and outside links open in
/// the user's browser, and notification/location requests are refused. The game's own
/// fullscreen button fills the screen. "link" games open in the user's browser directly.
#[tauri::command]
// Async on purpose: creating a window from a sync command deadlocks WebView2 on Windows
// (blank white window, both windows frozen).
async fn open_browser_game(app: AppHandle, st: State<'_, AppState>, id: String) -> Res<()> {
    use tauri::webview::{NewWindowResponse, PermissionKind, PermissionResponse};
    use tauri_plugin_opener::OpenerExt;
    let cat = st.cat();
    let g = cat.browser.iter().find(|b| b.id == id).ok_or("That game is not in the list.")?;
    let url: tauri::Url = g.url.parse().map_err(err)?;
    if g.embed != "window" {
        return app.opener().open_url(g.url.clone(), None::<&str>).map_err(err);
    }
    let label = format!("play-{}", id.replace(|c: char| !c.is_ascii_alphanumeric(), "-"));
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        return w.set_focus().map_err(err);
    }
    let home = site(&url);
    let (nav_app, popup_app) = (app.clone(), app.clone());
    let window = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(url))
        .title(&g.title)
        .inner_size(1280.0, 800.0)
        .min_inner_size(640.0, 400.0)
        .center()
        .zoom_hotkeys_enabled(false)
        .on_navigation(move |u| {
            let ok = matches!(u.scheme(), "about" | "blob" | "data") || site(u) == home;
            if !ok {
                let _ = nav_app.opener().open_url(u.as_str(), None::<&str>);
            }
            ok
        })
        .on_new_window(move |u, _| {
            let _ = popup_app.opener().open_url(u.as_str(), None::<&str>);
            NewWindowResponse::Deny
        })
        .on_permission_request(|_, kind| match kind {
            PermissionKind::Notifications | PermissionKind::Geolocation => PermissionResponse::Deny,
            _ => PermissionResponse::Default,
        })
        .build()
        .map_err(err)?;
    #[cfg(windows)]
    if let Err(e) = follow_page_fullscreen(&window) {
        eprintln!("{label}: the game's fullscreen button will only fill the window: {e}");
    }
    #[cfg(not(windows))]
    drop(window);
    Ok(())
}

/// WebView2 answers a page's fullscreen request (a game's fullscreen button) by filling the
/// window, not the screen. Make the window fullscreen while the page shows something fullscreen,
/// and leave fullscreen when the page does (Esc, or the game's own button).
#[cfg(windows)]
fn follow_page_fullscreen(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use std::sync::atomic::AtomicBool;
    use webview2_com::ContainsFullScreenElementChangedEventHandler;
    let target = window.clone();
    window.with_webview(move |webview| {
        // SAFETY: runs on the main thread with the window's live WebView2 controller.
        let core = match unsafe { webview.controller().CoreWebView2() } {
            Ok(core) => core,
            Err(e) => return eprintln!("fullscreen: no WebView2: {e}"),
        };
        // Changes are applied from another thread, so the window is never resized from inside
        // a WebView2 callback; the latest wish wins if two arrive close together.
        let wanted = Arc::new(AtomicBool::new(false));
        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(sender) = sender else { return Ok(()) };
            let mut on = Default::default();
            // SAFETY: `sender` is the WebView2 that raised the event, on its own thread.
            unsafe { sender.ContainsFullScreenElement(&mut on)? };
            wanted.store(on.as_bool(), Ordering::SeqCst);
            let (window, wanted) = (target.clone(), wanted.clone());
            std::thread::spawn(move || {
                let _ = window.set_fullscreen(wanted.load(Ordering::SeqCst));
            });
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: as above; the handler lives as long as the WebView2 holds it.
        if let Err(e) = unsafe { core.add_ContainsFullScreenElementChanged(&handler, &mut token) } {
            eprintln!("fullscreen: {e}");
        }
    })
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

// ---- AnyPS5: PS5 games converted to native Windows programs ----------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Anyps5Status {
    /// Installed AnyPS5 version, if any.
    tool: Option<String>,
    /// This game already has a converted program.
    converted: bool,
}

#[tauri::command]
fn anyps5_status(st: State<AppState>, id: i64) -> Res<Anyps5Status> {
    let (db, root) = (st.db()?, st.root()?);
    Ok(Anyps5Status { tool: onionring_core::anyps5::installed(&db, &root), converted: onionring_core::anyps5::converted(&db, id).is_some() })
}

/// Convert a PS5 game with AnyPS5 (installing AnyPS5 first if needed). Progress arrives as
/// "install" events with id "anyps5"; the final event has done = false so the UI shows its own
/// message (this is a conversion, not an install).
#[tauri::command]
async fn anyps5_convert(app: AppHandle, id: i64) -> Res<()> {
    blocking(move || {
        let st = app.state::<AppState>();
        let (db, root) = (st.db()?, st.root()?);
        let game = db.game(id).map_err(err)?;
        let report = |m: &str| {
            let _ = app.emit("install", InstallEvent { id: "anyps5".into(), message: m.to_string(), done: false, error: None });
        };
        let _guard = st.work.lock().unwrap();
        onionring_core::anyps5::convert(&db, &root, &game, &report).map_err(err)?;
        let _ = app.emit("library-changed", ());
        Ok(())
    })
    .await
}

/// Forget a converted program, so Play goes back to the PS5 emulator.
#[tauri::command]
fn anyps5_forget(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    st.db()?.kv_set(&onionring_core::anyps5::kv_key(id), "").map_err(err)?;
    let _ = app.emit("library-changed", ());
    Ok(())
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

/// Every update that can be installed: apps OnionRing installed (`managed`, update with
/// `install_latest`) and emulators found on the PC (the player updates them from `url`).
/// Slow the first time (asks GitHub), then cached.
#[tauri::command]
async fn available_updates(app: AppHandle) -> Res<Vec<AvailableUpdate>> {
    blocking(move || {
        let st = app.state::<AppState>();
        updates::available_updates(&st.root()?, &st.cat(), &st.db()?).map_err(err)
    })
    .await
}

// ---- save backups --------------------------------------------------------------------------

/// The save folders a backup would cover.
#[tauri::command]
async fn save_sources(app: AppHandle) -> Res<Vec<SaveSource>> {
    blocking(move || {
        let st = app.state::<AppState>();
        backups::save_sources(&st.root()?, &st.db()?, &st.cat()).map_err(err)
    })
    .await
}

/// Zip every save folder into a new backup. Reports progress like a scan (stage "backup").
#[tauri::command]
async fn back_up_saves(app: AppHandle) -> Res<Backup> {
    blocking(move || {
        let st = app.state::<AppState>();
        let _guard = match st.backup.try_lock() {
            Ok(g) => g,
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
            Err(TryLockError::WouldBlock) => return Err("A backup is already running.".into()),
        };
        let progress = emit_progress(&app);
        let result = (|| backups::backup_all(&st.root()?, &st.db()?, &st.cat(), &progress).map_err(err))();
        progress(Progress::new("idle", "", 1, 1));
        let backup = result?;
        let _ = app.emit("backups-changed", ());
        Ok(backup)
    })
    .await
}

/// Past backups, newest first.
#[tauri::command]
async fn backups(app: AppHandle) -> Res<Vec<Backup>> {
    blocking(move || Ok(backups::list_backups(&app.state::<AppState>().root()?))).await
}

// ---- auto-pickup ---------------------------------------------------------------------------
//
// While it is on, every reachable source folder is watched. A change that can add, remove or
// change a game (watch::should_rescan) starts a scan once things have been quiet for SETTLE,
// so copying a game scans once, after the copy. Scans wait while another scan or an install
// runs, or a game is being played. Drives coming and going are noticed within REACH_CHECK.

/// kv key; "0" is off, anything else (or nothing) is on.
const AUTO_SCAN_KEY: &str = "auto_scan";
/// Quiet time after the last relevant change before scanning.
const SETTLE: Duration = Duration::from_secs(3);
/// When busy (scan, install or game running), try again after this long.
const RETRY: Duration = Duration::from_secs(5);
/// How often to look for source folders that appeared or disappeared (a drive plugged in).
const REACH_CHECK: Duration = Duration::from_secs(30);
/// After the app starts, catch up on changes made while it was closed.
const STARTUP_DELAY: Duration = Duration::from_secs(8);

fn auto_scan_on(db: &Db) -> bool {
    db.kv_get(AUTO_SCAN_KEY).ok().flatten().as_deref() != Some("0")
}

#[tauri::command]
fn auto_scan(st: State<AppState>) -> Res<bool> {
    Ok(auto_scan_on(&st.db()?))
}

/// Turn auto-pickup on or off. Turning it on also scans once, for changes made while it was off.
#[tauri::command]
fn set_auto_scan(app: AppHandle, st: State<AppState>, on: bool) -> Res<()> {
    st.db()?.kv_set(AUTO_SCAN_KEY, if on { "1" } else { "0" }).map_err(err)?;
    rewatch_after(&app, on.then_some(SETTLE));
    Ok(())
}

/// Start watching again (sources or the data folder changed), off the calling thread.
fn rewatch(app: &AppHandle) {
    rewatch_after(app, None);
}

/// Start watching again and, with `scan_in`, scan after that long whatever changes.
fn rewatch_after(app: &AppHandle, scan_in: Option<Duration>) {
    let app = app.clone();
    std::thread::spawn(move || restart_watcher(&app, scan_in));
}

/// At startup: watch, and once the app has settled, scan for games added while it was closed
/// (only for a library that has been scanned before; first-run setup does its own scan).
fn start_watching(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let scanned = app.state::<AppState>().db().ok().and_then(|db| db.kv_get("last_scan").ok().flatten()).is_some();
        restart_watcher(&app, scanned.then_some(STARTUP_DELAY));
    });
}

type Stamped = (Instant, notify::Result<notify::Event>);

/// Replace the watcher. `scan_in`: also scan after this long, whatever changes.
fn restart_watcher(app: &AppHandle, scan_in: Option<Duration>) {
    use notify::{RecursiveMode, Watcher};
    let st = app.state::<AppState>();
    let mut slot = lock(&st.watcher);
    *slot = None; // the old loop stops when its channel closes or it sees the new generation
    let generation = st.watch_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let (Ok(root), Ok(db)) = (st.root(), st.db()) else { return };
    if !auto_scan_on(&db) {
        return;
    }
    let all: Vec<PathBuf> = db.sources().unwrap_or_default().into_iter().map(|s| PathBuf::from(s.path)).collect();
    let online = watch::watch_folders(&db).unwrap_or_default();
    drop(db);
    let (tx, rx) = std::sync::mpsc::channel::<Stamped>();
    // Stamp each change as it happens: a scan that starts later has already seen it.
    let mut watcher = match notify::recommended_watcher(move |ev| {
        let _ = tx.send((Instant::now(), ev));
    }) {
        Ok(w) => w,
        Err(e) => return eprintln!("auto-pickup unavailable: {e}"),
    };
    for folder in &online {
        if let Err(e) = watcher.watch(folder, RecursiveMode::Recursive) {
            eprintln!("auto-pickup: cannot watch {}: {e}", folder.display());
        }
    }
    *slot = Some(watcher);
    drop(slot);
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("auto-pickup".into())
        .spawn(move || watch_loop(&app, generation, &root, all, online, rx, scan_in));
    if let Err(e) = spawned {
        eprintln!("auto-pickup: {e}");
    }
}

fn reachable(all: &[PathBuf]) -> Vec<PathBuf> {
    all.iter().filter(|p| p.is_dir()).cloned().collect()
}

fn watch_loop(
    app: &AppHandle,
    generation: u64,
    root: &DataRoot,
    all: Vec<PathBuf>,
    online: Vec<PathBuf>,
    rx: Receiver<Stamped>,
    scan_in: Option<Duration>,
) {
    let st = app.state::<AppState>();
    let current = || st.watch_gen.load(Ordering::SeqCst) == generation;
    let own = own_folders(root);
    let mut due = scan_in.map(|d| Instant::now() + d);
    // When the newest change to catch up on happened. A catch-up scan (startup, a drive came
    // back) counts from now, so a scan someone starts before it is due covers it.
    let mut changed: Option<Instant> = scan_in.map(|_| Instant::now());
    let mut next_check = Instant::now() + REACH_CHECK;
    while current() {
        let wake = due.map_or(next_check, |d| d.min(next_check));
        match rx.recv_timeout(wake.saturating_duration_since(Instant::now())) {
            Ok((at, Ok(event))) => {
                if relevant(&st.cat(), &online, &own, &event) {
                    changed = Some(at);
                    due = Some(Instant::now() + SETTLE);
                }
            }
            // A watched folder failed (its drive went away?): look now.
            Ok((_, Err(_))) => next_check = Instant::now(),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if !current() {
            return;
        }
        let now = Instant::now();
        if now >= next_check {
            next_check = now + REACH_CHECK;
            if reachable(&all) != online {
                // A drive came or went: watch what is there now, and scan.
                return restart_watcher(app, Some(SETTLE));
            }
        }
        if due.is_some_and(|d| now >= d) {
            due = if scan_for_watcher(app, changed) { None } else { Some(Instant::now() + RETRY) };
        }
    }
}

/// Scan for the watcher. False when it has to wait: another scan or an install is running, or
/// a game is being played (a scan reads the disk hard, which could make the game stutter).
fn scan_for_watcher(app: &AppHandle, changed: Option<Instant>) -> bool {
    let st = app.state::<AppState>();
    if st.running.load(Ordering::SeqCst) > 0 {
        return false;
    }
    let _guard = match st.work.try_lock() {
        Ok(g) => g,
        Err(TryLockError::Poisoned(p)) => p.into_inner(),
        Err(TryLockError::WouldBlock) => return false,
    };
    // A scan (manual, after an install or unpack) that started after the change has seen it.
    let started = *lock(&st.scan_started);
    if let (Some(changed), Some(started)) = (changed, started)
        && started > changed
    {
        return true;
    }
    match scan_locked(app) {
        Ok(summary) => {
            let _ = app.emit("auto-scan", summary);
        }
        Err(e) => eprintln!("automatic scan failed: {e}"),
    }
    true
}

fn norm_path(p: &Path) -> String {
    util::path_str(p).trim_start_matches(r"\\?\").replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// Folders OnionRing writes to itself (downloads, installs, art, backups). Changes there never
/// start a scan, even when the data folder sits inside a source folder.
fn own_folders(root: &DataRoot) -> Vec<String> {
    [root.apps(), root.downloads(), root.media(), root.cache(), root.reclaim(), root.backups()]
        .iter()
        .map(|p| norm_path(p))
        .collect()
}

/// Whether a reported change can add, remove or change a game.
fn relevant(cat: &Catalog, sources: &[PathBuf], own: &[String], event: &notify::Event) -> bool {
    use notify::EventKind;
    use notify::event::ModifyKind;
    if event.need_rescan() {
        return true; // too many changes at once to report them all
    }
    event.paths.iter().any(|path| {
        let p = norm_path(path);
        if own.iter().any(|f| p.strip_prefix(f.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('\\'))) {
            return false;
        }
        let kind = match event.kind {
            EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => PathKind::probe(path),
            EventKind::Remove(_) => PathKind::Unknown,
            // Written to. Only a file counts: Windows also reports a folder as modified whenever
            // something inside it changes (an emulator writing a log, say).
            EventKind::Modify(_) => match PathKind::probe(path) {
                PathKind::File => PathKind::File,
                _ => return false,
            },
            _ => return false,
        };
        watch::should_rescan(cat, sources, path, kind)
    })
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

/// Open a folder in File Explorer (a setup check's folder, a backup). Folders only, so this can
/// never start a program.
#[tauri::command]
fn open_folder(app: AppHandle, path: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    if !Path::new(&path).is_dir() {
        return Err("That folder is not there any more.".into());
    }
    app.opener().open_path(path, None::<&str>).map_err(err)
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
            backup: Mutex::new(()),
            running: AtomicUsize::new(0),
            scan_started: Mutex::new(None),
            watcher: Mutex::new(None),
            watch_gen: AtomicU64::new(0),
        })
        .setup(|app| {
            if let Some(path) = AppConfig::load().data_root {
                match DataRoot::open(path) {
                    Ok(root) => {
                        use_root(app.handle(), root)?;
                        start_watching(app.handle());
                    }
                    Err(e) => eprintln!("data root unavailable: {e:#}"),
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            anyps5_status,
            anyps5_convert,
            anyps5_forget,
            set_data_root,
            sources,
            add_source,
            remove_source,
            games,
            game_copies,
            set_flag,
            set_game_emulator,
            set_game_args,
            set_cover,
            scan_library,
            refresh_art,
            launch_game,
            configure_port,
            unpack_game,
            collections,
            create_collection,
            rename_collection,
            delete_collection,
            add_to_collection,
            remove_from_collection,
            game_sessions,
            weekly_playtime,
            play_totals,
            top_games,
            emulators,
            set_emulator_args,
            setup_checks,
            catalog,
            install_latest,
            install_from_file,
            check_updates,
            available_updates,
            save_sources,
            back_up_saves,
            backups,
            auto_scan,
            set_auto_scan,
            storage_report,
            reclaim,
            restore_reclaimed,
            reveal,
            open_folder,
            open_browser_game,
            add_browser_game,
            remove_browser_game,
            picture,
            release_details,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OnionRing");
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::{RecursiveMode, Watcher};

    /// What a real watcher reports for `act`, run against a fresh source folder, and whether
    /// any of it would start a scan.
    fn triggers(name: &str, prepare: impl FnOnce(&Path), act: impl FnOnce(&Path)) -> bool {
        let dir = std::env::temp_dir().join(format!("onionring-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("Emulation");
        std::fs::create_dir_all(src.join("PS2").join("pcsx2")).unwrap();
        prepare(&src);
        let root = DataRoot { root: src.join("OnionRing") };
        let own = own_folders(&root);
        let cat = Catalog::load(None).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut w = notify::recommended_watcher(tx).unwrap();
        w.watch(&src, RecursiveMode::Recursive).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        act(&src);
        std::thread::sleep(Duration::from_millis(800));
        drop(w);
        let sources = vec![src.clone()];
        let hit = rx.try_iter().filter_map(Result::ok).any(|e| relevant(&cat, &sources, &own, &e));
        let _ = std::fs::remove_dir_all(&dir);
        hit
    }

    #[test]
    fn new_and_removed_games_start_a_scan() {
        assert!(triggers("iso", |_| {}, |s| std::fs::write(s.join("PS2").join("Okami (USA).iso"), b"x").unwrap()));
        assert!(triggers("folder", |_| {}, |s| std::fs::create_dir(s.join("PS2").join("Okami (USA)")).unwrap()));
        assert!(triggers(
            "rename",
            |s| std::fs::write(s.join("PS2").join("Okami.iso.crdownload"), b"x").unwrap(),
            |s| std::fs::rename(s.join("PS2").join("Okami.iso.crdownload"), s.join("PS2").join("Okami.iso")).unwrap()
        ));
        assert!(triggers(
            "remove",
            |s| std::fs::write(s.join("PS2").join("Okami.iso"), b"x").unwrap(),
            |s| std::fs::remove_file(s.join("PS2").join("Okami.iso")).unwrap()
        ));
    }

    #[test]
    fn emulator_housekeeping_and_own_files_do_not() {
        // A log or settings file written beside an emulator: its folder is reported as modified.
        assert!(!triggers("log", |_| {}, |s| std::fs::write(s.join("PS2").join("pcsx2").join("emulog.txt"), b"x").unwrap()));
        assert!(!triggers(
            "ini",
            |s| std::fs::write(s.join("PS2").join("pcsx2").join("PCSX2.ini"), b"a").unwrap(),
            |s| std::fs::write(s.join("PS2").join("pcsx2").join("PCSX2.ini"), b"b").unwrap()
        ));
        // A part-finished download.
        assert!(!triggers("part", |_| {}, |s| std::fs::write(s.join("PS2").join("Okami.iso.part"), b"x").unwrap()));
        // OnionRing's own folders, with the data folder inside a source.
        assert!(!triggers(
            "own",
            |s| std::fs::create_dir_all(s.join("OnionRing").join("Downloads")).unwrap(),
            |s| {
                std::fs::create_dir_all(s.join("OnionRing").join("Media").join("ab12")).unwrap();
                std::fs::write(s.join("OnionRing").join("Downloads").join("port.zip"), b"x").unwrap();
            }
        ));
    }
}
