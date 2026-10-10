//! OnionRing core: everything that is not UI. Used by the desktop app and by `onionring-cli`.

pub mod art;
pub mod backups;
pub mod catalog;
pub mod checks;
pub mod db;
pub mod disc;
pub mod github;
pub mod install;
pub mod launch;
pub mod model;
pub mod paths;
pub mod pictures;
pub mod scan;
pub mod storage;
pub mod updates;
pub mod util;
pub mod watch;

pub use anyhow::{Error, Result};
