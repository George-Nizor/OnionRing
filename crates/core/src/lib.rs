//! OnionRing core: everything that is not UI. Used by the desktop app and by `onionring-cli`.

pub mod art;
pub mod catalog;
pub mod db;
pub mod disc;
pub mod github;
pub mod install;
pub mod launch;
pub mod model;
pub mod paths;
pub mod scan;
pub mod storage;
pub mod util;

pub use anyhow::{Error, Result};
