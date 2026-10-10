use crate::model::{Copy, Game, Source};
use crate::util::now;
use anyhow::Result;
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
  user_emulator TEXT
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
CREATE TABLE IF NOT EXISTS kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);
"#;

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
        (SELECT COUNT(*) FROM copies c WHERE c.game_key = g.key) AS ncopies";

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
}
