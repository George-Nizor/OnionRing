use crate::model::{Collection, Copy, Game, PlayTotals, Session, Source, TopGame, WeekTotal};
use crate::util::{now, week_start};
use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS sources(
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE COLLATE NOCASE,
  added_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS emulators(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  exe TEXT NOT NULL,
  version TEXT,
  args TEXT,
  managed INTEGER NOT NULL DEFAULT 0,
  seen_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS games(
  id INTEGER PRIMARY KEY,
  key TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  platform TEXT NOT NULL,
  kind TEXT NOT NULL,
  launch_path TEXT NOT NULL,
  emulator TEXT,
  app_id TEXT,
  serial TEXT,
  region TEXT,
  size INTEGER NOT NULL DEFAULT 0,
  favorite INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  packed INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL,
  seen_at INTEGER NOT NULL,
  last_played INTEGER,
  playtime INTEGER NOT NULL DEFAULT 0,
  play_count INTEGER NOT NULL DEFAULT 0,
  cover TEXT, cover_thumb TEXT, hero TEXT, logo TEXT,
  art_checked INTEGER NOT NULL DEFAULT 0,
  user_emulator TEXT,
  user_args TEXT
);
CREATE TABLE IF NOT EXISTS copies(
  id INTEGER PRIMARY KEY,
  game_key TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE COLLATE NOCASE,
  format TEXT NOT NULL,
  size INTEGER NOT NULL,
  archive INTEGER NOT NULL DEFAULT 0,
  is_primary INTEGER NOT NULL DEFAULT 0,
  registered INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS copies_game ON copies(game_key);
CREATE TABLE IF NOT EXISTS sessions(
  id INTEGER PRIMARY KEY,
  game_id INTEGER NOT NULL,
  started INTEGER NOT NULL,
  ended INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_game ON sessions(game_id, started);
CREATE INDEX IF NOT EXISTS sessions_started ON sessions(started);
CREATE TABLE IF NOT EXISTS kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS collections(
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS collection_games(
  collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  game_key TEXT NOT NULL,
  added INTEGER NOT NULL,
  PRIMARY KEY(collection_id, game_key)
);
"#;

/// Columns added after the first release. `CREATE TABLE IF NOT EXISTS` leaves an existing
/// table alone, so these are added to older libraries here: (table, column, declaration).
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[("games", "user_emulator", "TEXT"), ("games", "user_args", "TEXT")];

fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut st = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names: Vec<String> = st.query_map([], |r| r.get(1))?.collect::<rusqlite::Result<_>>()?;
    if !names.iter().any(|n| n.eq_ignore_ascii_case(column)) {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
    }
    Ok(())
}

const WEEK: i64 = 7 * 86_400;

fn collection_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Give the collection a name.");
    }
    if name.chars().count() > 80 {
        bail!("That name is too long. Keep it under 80 characters.");
    }
    Ok(name)
}

/// What a scan knows about a game; user data (favourites, playtime, art) is preserved on upsert.
pub struct GameUpsert {
    pub key: String,
    pub title: String,
    pub platform: String,
    pub kind: String,
    pub launch_path: String,
    pub emulator: Option<String>,
    pub app_id: Option<String>,
    pub serial: Option<String>,
    pub region: Option<String>,
    pub size: i64,
    pub packed: bool,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        for (table, column, decl) in ADDED_COLUMNS {
            ensure_column(&conn, table, column, decl)?;
        }
        Ok(Db { conn })
    }

    // ---- sources -------------------------------------------------------------------------

    pub fn sources(&self) -> Result<Vec<Source>> {
        let mut st = self.conn.prepare("SELECT id, path, added_at FROM sources ORDER BY path")?;
        let rows = st.query_map([], |r| {
            let path: String = r.get(1)?;
            Ok(Source { id: r.get(0)?, exists: Path::new(&path).is_dir(), path, added_at: r.get(2)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn add_source(&self, path: &str) -> Result<()> {
        self.conn.execute("INSERT OR IGNORE INTO sources(path, added_at) VALUES(?1, ?2)", params![path, now()])?;
        Ok(())
    }

    pub fn remove_source(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM sources WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- emulators -----------------------------------------------------------------------

    pub fn upsert_emulator(&self, id: &str, name: &str, exe: &str, version: Option<&str>, managed: bool, seen: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO emulators(id, name, exe, version, managed, seen_at) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET name=?2, exe=?3, version=?4, managed=?5, seen_at=?6",
            params![id, name, exe, version, managed, seen],
        )?;
        Ok(())
    }

    /// (id, name, exe, version, args override, managed, seen_at)
    #[allow(clippy::type_complexity)]
    pub fn emulator_rows(&self) -> Result<Vec<(String, String, String, Option<String>, Option<String>, bool, i64)>> {
        let mut st = self.conn.prepare("SELECT id, name, exe, version, args, managed, seen_at FROM emulators ORDER BY name")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_emulator_args(&self, id: &str, args: Option<&[String]>) -> Result<()> {
        let v = args.map(serde_json::to_string).transpose()?;
        self.conn.execute("UPDATE emulators SET args = ?2 WHERE id = ?1", params![id, v])?;
        Ok(())
    }

    pub fn set_emulator_exe(&self, id: &str, exe: &str) -> Result<()> {
        self.conn.execute("UPDATE emulators SET exe = ?2 WHERE id = ?1", params![id, exe])?;
        Ok(())
    }

    // ---- games ---------------------------------------------------------------------------

    /// Returns (id, newly added).
    pub fn upsert_game(&self, g: &GameUpsert, seen: i64) -> Result<(i64, bool)> {
        let existing: Option<i64> =
            self.conn.query_row("SELECT id FROM games WHERE key = ?1", [&g.key], |r| r.get(0)).optional()?;
        match existing {
            Some(id) => {
                self.conn.execute(
                    "UPDATE games SET title=?2, platform=?3, kind=?4, launch_path=?5, emulator=?6, app_id=?7,
                     serial=COALESCE(?8, serial), region=COALESCE(?9, region), size=?10, packed=?11, seen_at=?12
                     WHERE id=?1",
                    params![id, g.title, g.platform, g.kind, g.launch_path, g.emulator, g.app_id, g.serial, g.region, g.size, g.packed, seen],
                )?;
                Ok((id, false))
            }
            None => {
                self.conn.execute(
                    "INSERT INTO games(key, title, platform, kind, launch_path, emulator, app_id, serial, region, size, packed, added_at, seen_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
                    params![g.key, g.title, g.platform, g.kind, g.launch_path, g.emulator, g.app_id, g.serial, g.region, g.size, g.packed, seen],
                )?;
                Ok((self.conn.last_insert_rowid(), true))
            }
        }
    }

    pub fn replace_copies(&self, key: &str, copies: &[Copy]) -> Result<()> {
        self.conn.execute("DELETE FROM copies WHERE game_key = ?1", [key])?;
        let mut st = self.conn.prepare(
            "INSERT OR REPLACE INTO copies(game_key, path, format, size, archive, is_primary, registered) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        )?;
        for c in copies {
            st.execute(params![key, c.path, c.format, c.size, c.archive, c.primary, c.registered])?;
        }
        Ok(())
    }

    pub fn copies(&self, key: &str) -> Result<Vec<Copy>> {
        let mut st = self.conn.prepare(
            "SELECT path, format, size, archive, is_primary, registered FROM copies WHERE game_key = ?1 ORDER BY is_primary DESC, archive, path",
        )?;
        let rows = st.query_map([key], |r| {
            Ok(Copy { path: r.get(0)?, format: r.get(1)?, size: r.get(2)?, archive: r.get(3)?, primary: r.get(4)?, registered: r.get(5)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Games of the given kind not seen since `since` lose their copies (they are shown as missing).
    pub fn sweep_unseen(&self, kind: &str, since: i64) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM copies WHERE game_key IN (SELECT key FROM games WHERE kind = ?1 AND seen_at < ?2)",
            params![kind, since],
        )?;
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM games WHERE kind = ?1 AND seen_at < ?2", params![kind, since], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Drop one game and its play history (something OnionRing installed was removed).
    pub fn forget_game(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM sessions WHERE game_id IN (SELECT id FROM games WHERE key = ?1)", [key])?;
        self.conn.execute("DELETE FROM games WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Drop ROM games that have no copies left, when `gone(launch_path)` confirms the files are
    /// really gone (their source folder is online). Games on an unplugged drive are kept.
    pub fn forget_missing(&self, gone: impl Fn(&str) -> bool) -> Result<usize> {
        let mut st = self.conn.prepare(
            "SELECT id, launch_path FROM games WHERE kind = 'rom' AND NOT EXISTS (SELECT 1 FROM copies c WHERE c.game_key = games.key)",
        )?;
        let rows: Vec<(i64, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut n = 0;
        for (id, path) in rows {
            if gone(&path) {
                self.conn.execute("DELETE FROM sessions WHERE game_id = ?1", [id])?;
                self.conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
                n += 1;
            }
        }
        Ok(n)
    }

    const GAME_COLS: &'static str = "g.id, g.key, g.title, g.platform, g.kind, g.launch_path, COALESCE(g.user_emulator, g.emulator), g.app_id,
        g.serial, g.region, g.size, g.favorite, g.hidden, g.packed, g.added_at, g.last_played, g.playtime, g.play_count,
        g.cover, g.cover_thumb, g.hero, g.logo,
        (SELECT COUNT(*) FROM copies c WHERE c.game_key = g.key) AS ncopies, g.user_args";

    fn row_to_game(r: &rusqlite::Row) -> rusqlite::Result<Game> {
        let copies: i64 = r.get(22)?;
        let kind: String = r.get(4)?;
        Ok(Game {
            id: r.get(0)?,
            key: r.get(1)?,
            title: r.get(2)?,
            platform: r.get(3)?,
            missing: kind == "rom" && copies == 0,
            kind,
            launch_path: r.get(5)?,
            emulator: r.get(6)?,
            app_id: r.get(7)?,
            serial: r.get(8)?,
            region: r.get(9)?,
            size: r.get(10)?,
            favorite: r.get(11)?,
            hidden: r.get(12)?,
            packed: r.get(13)?,
            added_at: r.get(14)?,
            last_played: r.get(15)?,
            playtime: r.get(16)?,
            play_count: r.get(17)?,
            cover: r.get(18)?,
            cover_thumb: r.get(19)?,
            hero: r.get(20)?,
            logo: r.get(21)?,
            copies,
            user_args: r.get::<_, Option<String>>(23)?.and_then(|a| serde_json::from_str(&a).ok()),
        })
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        let sql = format!("SELECT {} FROM games g ORDER BY g.title COLLATE NOCASE", Self::GAME_COLS);
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([], Self::row_to_game)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn game(&self, id: i64) -> Result<Game> {
        let sql = format!("SELECT {} FROM games g WHERE g.id = ?1", Self::GAME_COLS);
        Ok(self.conn.query_row(&sql, [id], Self::row_to_game)?)
    }

    pub fn set_flag(&self, id: i64, flag: &str, on: bool) -> Result<()> {
        let col = match flag {
            "favorite" => "favorite",
            "hidden" => "hidden",
            _ => anyhow::bail!("unknown flag {flag}"),
        };
        self.conn.execute(&format!("UPDATE games SET {col} = ?2 WHERE id = ?1"), params![id, on])?;
        Ok(())
    }

    pub fn set_user_emulator(&self, id: i64, emu: Option<&str>) -> Result<()> {
        self.conn.execute("UPDATE games SET user_emulator = ?2 WHERE id = ?1", params![id, emu])?;
        Ok(())
    }

    /// Launch options for one game (one argv token per entry). `None` or an empty list goes
    /// back to the emulator's own options.
    pub fn set_game_args(&self, id: i64, args: Option<&[String]>) -> Result<()> {
        let v = args.filter(|a| !a.is_empty()).map(serde_json::to_string).transpose()?;
        self.conn.execute("UPDATE games SET user_args = ?2 WHERE id = ?1", params![id, v])?;
        Ok(())
    }

    pub fn record_session(&self, id: i64, started: i64, ended: i64) -> Result<()> {
        let secs = (ended - started).max(0);
        self.conn.execute("INSERT INTO sessions(game_id, started, ended) VALUES(?1,?2,?3)", params![id, started, ended])?;
        self.conn.execute(
            "UPDATE games SET playtime = playtime + ?2, play_count = play_count + 1, last_played = ?3 WHERE id = ?1",
            params![id, secs, started],
        )?;
        Ok(())
    }

    pub fn mark_played(&self, id: i64, at: i64) -> Result<()> {
        self.conn.execute("UPDATE games SET last_played = ?2 WHERE id = ?1", params![id, at])?;
        Ok(())
    }

    /// (id, key, title, platform, kind, launch_path, app_id, serial)
    #[allow(clippy::type_complexity)]
    pub fn art_pending(&self, force: bool) -> Result<Vec<(i64, String, String, String, String, String, Option<String>)>> {
        let sql = if force {
            "SELECT id, key, title, platform, kind, launch_path, app_id FROM games"
        } else {
            "SELECT id, key, title, platform, kind, launch_path, app_id FROM games WHERE art_checked = 0"
        };
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Forget fetched art before a forced refresh (custom covers are kept).
    pub fn reset_art(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET hero = NULL, logo = NULL, art_checked = 0,
             cover = CASE WHEN cover LIKE '%cover_custom%' THEN cover END,
             cover_thumb = CASE WHEN cover LIKE '%cover_custom%' THEN cover_thumb END",
            [],
        )?;
        Ok(())
    }

    pub fn set_art(&self, id: i64, cover: Option<&str>, thumb: Option<&str>, hero: Option<&str>, logo: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover = COALESCE(?2, cover), cover_thumb = COALESCE(?3, cover_thumb),
             hero = COALESCE(?4, hero), logo = COALESCE(?5, logo), art_checked = 1 WHERE id = ?1",
            params![id, cover, thumb, hero, logo],
        )?;
        Ok(())
    }

    pub fn kv_get(&self, k: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT v FROM kv WHERE k = ?1", [k], |r| r.get(0)).optional()?)
    }

    pub fn kv_set(&self, k: &str, v: &str) -> Result<()> {
        self.conn.execute("INSERT INTO kv(k, v) VALUES(?1, ?2) ON CONFLICT(k) DO UPDATE SET v = ?2", params![k, v])?;
        Ok(())
    }

    /// All copies grouped by game, for the storage report: (game_key, title, platform, copy).
    pub fn all_copies(&self) -> Result<Vec<(String, String, String, Copy)>> {
        let mut st = self.conn.prepare(
            "SELECT c.game_key, g.title, g.platform, c.path, c.format, c.size, c.archive, c.is_primary, c.registered
             FROM copies c JOIN games g ON g.key = c.game_key ORDER BY g.title COLLATE NOCASE, c.is_primary DESC",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                Copy { path: r.get(3)?, format: r.get(4)?, size: r.get(5)?, archive: r.get(6)?, primary: r.get(7)?, registered: r.get(8)? },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_copy(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM copies WHERE path = ?1", [path])?;
        Ok(())
    }

    // ---- collections ---------------------------------------------------------------------

    /// Every collection, by name, each with the ids of its games that are in the library.
    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut st = self.conn.prepare("SELECT id, name, created FROM collections ORDER BY name COLLATE NOCASE, id")?;
        let mut out: Vec<Collection> = st
            .query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, created: r.get(2)?, game_ids: vec![] }))?
            .collect::<rusqlite::Result<_>>()?;
        let mut st = self.conn.prepare(
            "SELECT cg.collection_id, g.id FROM collection_games cg JOIN games g ON g.key = cg.game_key
             ORDER BY cg.added, cg.rowid",
        )?;
        let members = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for m in members {
            let (cid, gid) = m?;
            if let Some(c) = out.iter_mut().find(|c| c.id == cid) {
                c.game_ids.push(gid);
            }
        }
        Ok(out)
    }

    fn collection_taken(&self, name: &str, except: Option<i64>) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM collections WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2",
            params![name, except],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn create_collection(&self, name: &str) -> Result<Collection> {
        let name = collection_name(name)?;
        if self.collection_taken(name, None)? {
            bail!("You already have a collection called {name}.");
        }
        let created = now();
        self.conn.execute("INSERT INTO collections(name, created) VALUES(?1, ?2)", params![name, created])?;
        Ok(Collection { id: self.conn.last_insert_rowid(), name: name.to_string(), created, game_ids: vec![] })
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> Result<()> {
        let name = collection_name(name)?;
        if self.collection_taken(name, Some(id))? {
            bail!("You already have a collection called {name}.");
        }
        if self.conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name])? == 0 {
            bail!("That collection no longer exists.");
        }
        Ok(())
    }

    /// Delete a collection. Its games stay in the library.
    pub fn delete_collection(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM collection_games WHERE collection_id = ?1", [id])?;
        self.conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Add a game to a collection (adding it twice is fine).
    pub fn add_to_collection(&self, collection_id: i64, game_id: i64) -> Result<()> {
        let exists: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM collections WHERE id = ?1)", [collection_id], |r| r.get(0))?;
        if !exists {
            bail!("That collection no longer exists.");
        }
        let Some(key) = self.conn.query_row("SELECT key FROM games WHERE id = ?1", [game_id], |r| r.get::<_, String>(0)).optional()?
        else {
            bail!("That game is no longer in your library.");
        };
        self.conn.execute(
            "INSERT OR IGNORE INTO collection_games(collection_id, game_key, added) VALUES(?1, ?2, ?3)",
            params![collection_id, key, now()],
        )?;
        Ok(())
    }

    pub fn remove_from_collection(&self, collection_id: i64, game_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM collection_games WHERE collection_id = ?1 AND game_key = (SELECT key FROM games WHERE id = ?2)",
            params![collection_id, game_id],
        )?;
        Ok(())
    }

    // ---- play history --------------------------------------------------------------------

    /// A game's sessions, newest first.
    pub fn game_sessions(&self, game_id: i64, limit: usize) -> Result<Vec<Session>> {
        let mut st = self.conn.prepare(
            "SELECT id, game_id, started, ended FROM sessions WHERE game_id = ?1 ORDER BY started DESC, id DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![game_id, limit as i64], |r| {
            let (started, ended): (i64, i64) = (r.get(2)?, r.get(3)?);
            Ok(Session { id: r.get(0)?, game_id: r.get(1)?, started, ended, seconds: (ended - started).max(0) })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Time played per week for the last `weeks` weeks including the current one, oldest
    /// first, with empty weeks included. Weeks start on Monday 00:00 in a zone `utc_offset`
    /// seconds east of UTC (`util::local_offset()`); a session counts towards the week it
    /// started in.
    pub fn weekly_playtime(&self, weeks: usize, now: i64, utc_offset: i64) -> Result<Vec<WeekTotal>> {
        if weeks == 0 {
            return Ok(vec![]);
        }
        let first = week_start(now, utc_offset) - (weeks as i64 - 1) * WEEK;
        let mut out: Vec<WeekTotal> =
            (0..weeks as i64).map(|i| WeekTotal { start: first + i * WEEK, seconds: 0, sessions: 0 }).collect();
        let mut st = self.conn.prepare("SELECT started, ended FROM sessions WHERE started >= ?1 AND started < ?2")?;
        let rows = st.query_map(params![first, first + weeks as i64 * WEEK], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (started, ended) = row?;
            // Daylight saving can shift a week's start by an hour; clamp instead of dropping.
            let i = ((week_start(started, utc_offset) - first) / WEEK).clamp(0, weeks as i64 - 1) as usize;
            out[i].seconds += (ended - started).max(0);
            out[i].sessions += 1;
        }
        Ok(out)
    }

    pub fn play_totals(&self) -> Result<PlayTotals> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(MAX(ended - started, 0)), 0), COUNT(*), COUNT(DISTINCT game_id) FROM sessions",
            [],
            |r| Ok(PlayTotals { seconds: r.get(0)?, sessions: r.get(1)?, games: r.get(2)? }),
        )?)
    }

    /// Games with the most time played, most first. `since` (a Unix time) limits it to recent
    /// sessions, e.g. this month.
    pub fn top_games(&self, limit: usize, since: Option<i64>) -> Result<Vec<TopGame>> {
        let mut st = self.conn.prepare(
            "SELECT s.game_id, g.title, g.platform, SUM(MAX(s.ended - s.started, 0)) AS secs, COUNT(*), MAX(s.started) AS last
             FROM sessions s JOIN games g ON g.id = s.game_id
             WHERE s.started >= ?1
             GROUP BY s.game_id ORDER BY secs DESC, last DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![since.unwrap_or(i64::MIN), limit as i64], |r| {
            Ok(TopGame {
                game_id: r.get(0)?,
                title: r.get(1)?,
                platform: r.get(2)?,
                seconds: r.get(3)?,
                sessions: r.get(4)?,
                last_played: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    fn memory() -> Db {
        Db::open(Path::new(":memory:")).unwrap()
    }

    fn add_game(db: &Db, key: &str, title: &str) -> i64 {
        let g = GameUpsert {
            key: key.into(),
            title: title.into(),
            platform: "ps2".into(),
            kind: "rom".into(),
            launch_path: format!("G:\\Games\\{title}.iso"),
            emulator: Some("pcsx2".into()),
            app_id: None,
            serial: None,
            region: None,
            size: 1,
            packed: false,
        };
        db.upsert_game(&g, 1).unwrap().0
    }

    #[test]
    fn migrates_an_old_library() {
        let t = TempDir::new("db-migrate");
        let file = t.path().join("library.db");
        {
            // A library from before user_args and collections existed.
            let c = Connection::open(&file).unwrap();
            c.execute_batch(
                "CREATE TABLE games(id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE, title TEXT NOT NULL, platform TEXT NOT NULL,
                   kind TEXT NOT NULL, launch_path TEXT NOT NULL, emulator TEXT, app_id TEXT, serial TEXT, region TEXT,
                   size INTEGER NOT NULL DEFAULT 0, favorite INTEGER NOT NULL DEFAULT 0, hidden INTEGER NOT NULL DEFAULT 0,
                   packed INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL, seen_at INTEGER NOT NULL, last_played INTEGER,
                   playtime INTEGER NOT NULL DEFAULT 0, play_count INTEGER NOT NULL DEFAULT 0,
                   cover TEXT, cover_thumb TEXT, hero TEXT, logo TEXT, art_checked INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO games(key, title, platform, kind, launch_path, added_at, seen_at, favorite)
                   VALUES('ps2:okami', 'Okami', 'ps2', 'rom', 'G:\\Okami.iso', 1, 1, 1);",
            )
            .unwrap();
        }
        let db = Db::open(&file).unwrap();
        let g = &db.games().unwrap()[0];
        assert!(g.favorite && g.user_args.is_none() && g.emulator.is_none());
        db.set_game_args(g.id, Some(&["-fastboot".into(), "{rom}".into()])).unwrap();
        db.set_user_emulator(g.id, Some("pcsx2")).unwrap();
        drop(db);
        // Opening again is a no-op and keeps everything.
        let db = Db::open(&file).unwrap();
        let g = &db.games().unwrap()[0];
        assert_eq!(g.user_args.as_deref(), Some(&["-fastboot".to_string(), "{rom}".to_string()][..]));
        assert_eq!(g.emulator.as_deref(), Some("pcsx2"));
        assert!(db.collections().unwrap().is_empty());
    }

    #[test]
    fn game_args_reset() {
        let db = memory();
        let id = add_game(&db, "ps2:okami", "Okami");
        db.set_game_args(id, Some(&["{rom}".into()])).unwrap();
        assert!(db.game(id).unwrap().user_args.is_some());
        db.set_game_args(id, Some(&[])).unwrap();
        assert!(db.game(id).unwrap().user_args.is_none());
        db.set_game_args(id, Some(&["{rom}".into()])).unwrap();
        db.set_game_args(id, None).unwrap();
        assert!(db.game(id).unwrap().user_args.is_none());
    }

    #[test]
    fn collections() {
        let db = memory();
        let okami = add_game(&db, "ps2:okami", "Okami");
        let ico = add_game(&db, "ps2:ico", "Ico");
        let rpg = db.create_collection("  RPGs ").unwrap();
        assert_eq!(rpg.name, "RPGs");
        assert!(db.create_collection("rpgs").is_err(), "names are unique, any case");
        assert!(db.create_collection("   ").is_err());
        let other = db.create_collection("Shooters").unwrap();

        db.add_to_collection(rpg.id, okami).unwrap();
        db.add_to_collection(rpg.id, ico).unwrap();
        db.add_to_collection(rpg.id, okami).unwrap(); // twice is fine
        db.add_to_collection(other.id, ico).unwrap();
        assert!(db.add_to_collection(rpg.id, 999).is_err());
        assert!(db.add_to_collection(999, okami).is_err());

        let all = db.collections().unwrap();
        assert_eq!(all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["RPGs", "Shooters"]);
        assert_eq!(all[0].game_ids, vec![okami, ico]);

        db.remove_from_collection(rpg.id, okami).unwrap();
        assert_eq!(db.collections().unwrap()[0].game_ids, vec![ico]);

        assert!(db.rename_collection(other.id, "RPGS").is_err());
        db.rename_collection(other.id, "Favourites").unwrap();
        db.rename_collection(other.id, "favourites").unwrap(); // its own name in another case
        assert!(db.rename_collection(999, "Gone").is_err());
        assert_eq!(db.collections().unwrap()[0].name, "favourites");

        // A game that vanishes (drive unplugged) and comes back keeps its collections.
        db.conn.execute("DELETE FROM games WHERE id = ?1", [ico]).unwrap();
        assert!(db.collections().unwrap().iter().all(|c| c.game_ids.is_empty()));
        let ico_again = add_game(&db, "ps2:ico", "Ico");
        assert_eq!(db.collections().unwrap()[0].game_ids, vec![ico_again]);

        db.delete_collection(rpg.id).unwrap();
        assert_eq!(db.collections().unwrap().len(), 1);
        let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM collection_games WHERE collection_id = ?1", [rpg.id], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn play_history() {
        let db = memory();
        let okami = add_game(&db, "ps2:okami", "Okami");
        let ico = add_game(&db, "ps2:ico", "Ico");
        let monday = 1_704_067_200; // 2024-01-01 00:00 UTC
        let now = monday + 7 * 86_400 + 3 * 3600; // the following Monday, 03:00
        db.record_session(okami, monday - 86_400, monday - 86_400 + 600).unwrap(); // previous week (Sunday)
        db.record_session(okami, monday + 3600, monday + 3600 + 1800).unwrap();
        db.record_session(ico, monday + 2 * 86_400, monday + 2 * 86_400 + 3600).unwrap();
        db.record_session(okami, now - 3600, now - 1200).unwrap(); // this week
        db.record_session(ico, now - 100, now - 200).unwrap(); // clock went backwards: zero seconds

        let s = db.game_sessions(okami, 2).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].started, s[0].seconds), (now - 3600, 2400));
        assert_eq!(s[1].started, monday + 3600);

        let w = db.weekly_playtime(3, now, 0).unwrap();
        assert_eq!(w.iter().map(|w| w.start).collect::<Vec<_>>(), vec![monday - 7 * 86_400, monday, monday + 7 * 86_400]);
        assert_eq!(w.iter().map(|w| (w.seconds, w.sessions)).collect::<Vec<_>>(), vec![(600, 1), (5400, 2), (2400, 2)]);
        // Two hours west of UTC, the Sunday-night session is still in the week before, but
        // "now" (01:00 local on Monday) starts a new week too.
        assert_eq!(db.weekly_playtime(2, now, -2 * 3600).unwrap()[1].start, monday + 7 * 86_400 + 2 * 3600);
        assert!(db.weekly_playtime(0, now, 0).unwrap().is_empty());

        let t = db.play_totals().unwrap();
        assert_eq!(t, PlayTotals { seconds: 600 + 1800 + 3600 + 2400, sessions: 5, games: 2 });

        let top = db.top_games(10, None).unwrap();
        assert_eq!(top.iter().map(|g| (g.title.as_str(), g.seconds, g.sessions)).collect::<Vec<_>>(), [("Okami", 4800, 3), ("Ico", 3600, 2)]);
        assert_eq!(top[0].last_played, now - 3600);
        let recent = db.top_games(1, Some(monday + 86_400)).unwrap();
        assert_eq!(recent.iter().map(|g| (g.title.as_str(), g.seconds)).collect::<Vec<_>>(), [("Ico", 3600)]);
    }
}
